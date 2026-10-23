use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::{StreamDecoder, StreamEncoder};
use kaeso_core::assembler::Assembler;
use kaeso_core::estimate::{self, Tally};
use kaeso_core::provider::AccountSignIn;
use kaeso_kpp::{DialectId, KppError, Reply, StopReason, StreamEvent};
use tokio::sync::mpsc::Sender;
use tokio::time::{Instant, sleep_until};

use kaeso_upstream::UpstreamError;

const INCOMPLETE: &str = "incomplete";

use super::hooks::{SHUT_DOWN, STREAM_INTERRUPTED, STREAM_LIMIT, StreamProgress};
use super::input_count::InputCounter;
use super::pipeline::{
    EstimateReason, Recording, count_estimate, count_upstream_error, shutting_down, transport_kind,
    upstream_error,
};
use crate::services::continuity::StreamCarry;

pub(super) struct Keepalive {
    interval: Option<Duration>,
}

impl Keepalive {
    pub(super) fn new(interval: Duration) -> Keepalive {
        Keepalive {
            interval: (!interval.is_zero()).then_some(interval),
        }
    }

    fn deadline(&self, from: Instant) -> Option<Instant> {
        self.interval.map(|i| from + i)
    }
}

impl StreamProgress {
    fn count(&mut self, events: &[StreamEvent]) {
        let n = u32::try_from(events.len()).unwrap_or(u32::MAX);
        self.events = self.events.saturating_add(n);
    }
}

struct Broken {
    kind: String,
    status: u16,
    err: KppError,
}

impl Broken {
    fn transport(e: &UpstreamError, err: KppError) -> Broken {
        match e {
            UpstreamError::Timeout(_) => Broken {
                kind: err.kind.as_str().to_owned(),
                status: 504,
                err,
            },
            UpstreamError::Connect(_)
            | UpstreamError::Transfer(_)
            | UpstreamError::TooLarge(_)
            | UpstreamError::Client(_)
            | UpstreamError::Method(_) => Broken {
                kind: STREAM_INTERRUPTED.to_owned(),
                status: 502,
                err,
            },
        }
    }

    fn event(err: &KppError) -> Broken {
        Broken {
            kind: err.kind.as_str().to_owned(),
            status: err.status.filter(|s| *s >= 400).unwrap_or(502),
            err: err.clone(),
        }
    }

    fn ended(err: KppError) -> Broken {
        Broken {
            kind: STREAM_INTERRUPTED.to_owned(),
            status: 502,
            err,
        }
    }

    fn shutdown(err: KppError) -> Broken {
        Broken {
            kind: SHUT_DOWN.to_owned(),
            status: 503,
            err,
        }
    }

    fn capped(err: KppError) -> Broken {
        Broken {
            kind: STREAM_LIMIT.to_owned(),
            status: 502,
            err,
        }
    }

    fn encoder(err: KppError) -> Broken {
        Broken {
            kind: err.kind.as_str().to_owned(),
            status: err.status.filter(|s| *s >= 400).unwrap_or(500),
            err,
        }
    }
}

const SHUTDOWN_WRITE: Duration = Duration::from_secs(1);

struct ToApp {
    tx: Sender<Bytes>,
    progress: StreamProgress,
    arrived: std::time::Instant,
    state: crate::state::AppState,
    patience: Duration,
    stopping: bool,
}

impl ToApp {
    async fn deliver(&self, bytes: Bytes) -> bool {
        let patience = if self.stopping {
            SHUTDOWN_WRITE
        } else {
            self.patience
        };
        tokio::select! {
            sent = self.tx.send(bytes) => sent.is_ok(),
            () = tokio::time::sleep(patience) => {
                tracing::info!(waited = ?patience, "the app stopped reading a stream; ending it");
                false
            }
            () = self.state.in_flight.cut(), if !self.stopping => false,
        }
    }

    async fn reply(&mut self, bytes: Vec<u8>) -> bool {
        if bytes.is_empty() {
            return true;
        }
        let n = bytes.len() as u64;
        if !self.deliver(Bytes::from(bytes)).await {
            return false;
        }
        self.progress.bytes += n;
        let arrived = self.arrived;
        self.progress
            .first_byte
            .get_or_insert_with(|| arrived.elapsed());
        true
    }

    async fn keepalive(&mut self, bytes: Vec<u8>) -> bool {
        if bytes.is_empty() {
            return true;
        }
        let n = bytes.len() as u64;
        if !self.deliver(Bytes::from(bytes)).await {
            return false;
        }
        self.progress.bytes += n;
        self.progress.keepalives += 1;
        true
    }
}

fn encode(
    encoder: &mut dyn StreamEncoder,
    event: &StreamEvent,
    broken: &mut Option<Broken>,
) -> Result<Vec<u8>, Vec<u8>> {
    encoder.encode(event).map_err(|e| {
        tracing::error!(error = %e, "kpp cannot encode a stream event for the app");
        let bytes = encoder
            .encode(&StreamEvent::Error(e.clone()))
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "kpp cannot encode a stream error for the app");
                Vec::new()
            });
        broken.get_or_insert_with(|| Broken::encoder(e));
        bytes
    })
}

pub(super) struct Parts {
    pub(super) chunks: kaeso_upstream::ByteStream,
    pub(super) decoder: Box<dyn StreamDecoder>,
    pub(super) encoder: Box<dyn StreamEncoder>,
    pub(super) keepalive: Keepalive,
    pub(super) counter: InputCounter,
    pub(super) carry: Option<StreamCarry>,
    pub(super) keep_reply: bool,
    pub(super) account: Option<AccountSignIn>,
}

pub(super) enum Gathered {
    Transport(UpstreamError),
    Provider(KppError),
    Broken(KppError),
}

pub(super) async fn gather(
    mut chunks: kaeso_upstream::ByteStream,
    mut decoder: Box<dyn StreamDecoder>,
    dialect: DialectId,
    max: usize,
) -> Result<Reply, Gathered> {
    let mut asm = Assembler::new(dialect);
    let mut read = 0usize;
    loop {
        let (events, last) = match chunks.next().await {
            Some(Ok(bytes)) => {
                read = read.saturating_add(bytes.len());
                if read > max {
                    return Err(Gathered::Transport(UpstreamError::TooLarge(max)));
                }
                (decoder.feed(&bytes), false)
            }
            Some(Err(e)) => return Err(Gathered::Transport(e)),
            None => (decoder.finish(), true),
        };
        for event in &events {
            asm.feed(event);
        }
        if last {
            break;
        }
    }
    let assembled = asm.finish();
    if let Some(mut err) = assembled.error {
        super::pipeline::redact_key_fragments(&mut err);
        return Err(Gathered::Provider(err));
    }
    match assembled.response {
        Some(response) if assembled.complete => Ok(Reply::Generate(response)),
        _ => Err(Gathered::Broken(
            KppError::provider("The provider's stream ended before the reply was complete.")
                .with_status(502),
        )),
    }
}

const RESERVATION_REFRESH: time::Duration = time::Duration::seconds(60);

struct Reserved {
    request_id: String,
    due: Option<time::OffsetDateTime>,
}

impl Reserved {
    fn new(rec: &mut Recording) -> Reserved {
        let now = rec.state().clock.now();
        Reserved {
            request_id: rec.outcome().request_id.to_string(),
            due: rec
                .reservation()
                .is_held()
                .then(|| now + RESERVATION_REFRESH),
        }
    }

    async fn keep(&mut self, rec: &Recording) {
        let Some(due) = self.due else { return };
        let now = rec.state().clock.now();
        if now < due {
            return;
        }
        self.due = Some(now + RESERVATION_REFRESH);
        if let Err(e) =
            crate::services::limits::extend(rec.state(), rec.reservation(), &self.request_id).await
        {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "reservation")
                .increment(1);
            tracing::warn!(error = %e, "cannot extend a stream's reservation under its limits");
        }
    }
}

pub(super) async fn pump(parts: Parts, tx: Sender<Bytes>, mut rec: Recording) {
    let state = rec.state().clone();
    let mut reserved = Reserved::new(&mut rec);
    let Parts {
        mut chunks,
        mut decoder,
        mut encoder,
        keepalive,
        counter,
        mut carry,
        keep_reply,
        account,
    } = parts;
    let mut asm = if keep_reply {
        Assembler::new(counter.dialect)
    } else {
        Assembler::without_blocks(counter.dialect)
    };
    let mut tally = Tally::new();
    let mut app = ToApp {
        tx,
        progress: StreamProgress::default(),
        arrived: rec.arrived(),
        state: state.clone(),
        patience: state.config.upstream.stream_idle_timeout,
        stopping: false,
    };
    let limits = &state.config.streams;
    let max_bytes = limits.max_bytes.0;
    let mut gone = false;
    let mut broken: Option<Broken> = None;
    let mut last_write = Instant::now();
    {
        let watch = app.tx.clone();
        let closed = watch.closed();
        tokio::pin!(closed);
        let quiet = sleep_until(keepalive.deadline(last_write).unwrap_or(last_write));
        tokio::pin!(quiet);
        let cut = state.in_flight.cut();
        tokio::pin!(cut);
        let too_long = tokio::time::sleep(limits.max_duration);
        tokio::pin!(too_long);
        'read: loop {
            let next = tokio::select! {
                () = &mut closed => { gone = true; break 'read; }
                () = &mut too_long, if !limits.max_duration.is_zero() => {
                    if !end_capped(&mut app, encoder.as_mut(), &mut broken).await {
                        gone = true;
                    }
                    break 'read;
                }
                () = &mut cut => {
                    app.stopping = true;
                    let err = shutting_down();
                    broken.get_or_insert_with(|| Broken::shutdown(err.clone()));
                    let bytes = match encode(encoder.as_mut(), &StreamEvent::Error(err), &mut broken) {
                        Ok(bytes) | Err(bytes) => bytes,
                    };
                    if !app.reply(bytes).await {
                        gone = true;
                    }
                    break 'read;
                }
                next = chunks.next() => {
                    reserved.keep(&rec).await;
                    next
                }
                () = &mut quiet, if keepalive.interval.is_some() => {
                    if let Some(due) = keepalive.deadline(last_write)
                        && due > Instant::now()
                    {
                        quiet.as_mut().reset(due);
                        continue;
                    }
                    reserved.keep(&rec).await;
                    if !app.keepalive(encoder.keepalive()).await {
                        gone = true;
                        break 'read;
                    }
                    last_write = Instant::now();
                    if let Some(due) = keepalive.deadline(last_write) {
                        quiet.as_mut().reset(due);
                    }
                    continue;
                }
            };
            let (events, last) = match next {
                Some(Ok(bytes)) => {
                    let mut events = decoder.feed(&bytes);
                    app.progress.count(&events);
                    let mut failed = false;
                    for e in &mut events {
                        if let StreamEvent::Error(err) = e {
                            super::pipeline::redact_key_fragments(err);
                            if let Some(limited) = super::accounts::plan_limit(account, err) {
                                *err = limited;
                            }
                            broken.get_or_insert_with(|| Broken::event(err));
                            failed = true;
                        }
                    }
                    (events, failed)
                }
                Some(Err(e)) => {
                    count_upstream_error(
                        transport_kind(&e),
                        rec.outcome().provider.unwrap_or_default(),
                    );
                    let err = upstream_error(&e);
                    broken.get_or_insert_with(|| Broken::transport(&e, err.clone()));
                    (vec![StreamEvent::Error(err)], true)
                }
                None => {
                    let mut events = decoder.finish();
                    app.progress.count(&events);
                    for e in &mut events {
                        if let StreamEvent::Error(err) = e {
                            super::pipeline::redact_key_fragments(err);
                            broken.get_or_insert_with(|| Broken::ended(err.clone()));
                        }
                    }
                    (events, true)
                }
            };
            let mut out = Vec::new();
            let mut stop = false;
            for event in &events {
                asm.feed(event);
                tally.feed(event);
                if let Some(c) = carry.as_mut() {
                    c.observe(event);
                }
                let bytes = match encode(encoder.as_mut(), event, &mut broken) {
                    Ok(bytes) => bytes,
                    Err(bytes) => {
                        stop = true;
                        bytes
                    }
                };
                if out.is_empty() {
                    out = bytes;
                } else {
                    out.extend_from_slice(&bytes);
                }
                if stop {
                    break;
                }
            }
            if !out.is_empty() {
                if !app.reply(out).await {
                    gone = true;
                    break 'read;
                }
                last_write = Instant::now();
                if max_bytes > 0 && app.progress.bytes >= max_bytes {
                    if !end_capped(&mut app, encoder.as_mut(), &mut broken).await {
                        gone = true;
                    }
                    break 'read;
                }
            }
            if stop {
                break 'read;
            }
            if last {
                break;
            }
        }
    }
    drop(chunks);
    let mut assembled = asm.finish();
    if !assembled.complete
        && let Some(r) = assembled.response.as_mut()
    {
        r.stop = StopReason::Other(INCOMPLETE.to_owned());
    }
    if !gone && broken.is_none() && !assembled.complete {
        let err = KppError::provider("The provider's stream ended before the reply was complete.")
            .with_status(502);
        let bytes = match encode(
            encoder.as_mut(),
            &StreamEvent::Error(err.clone()),
            &mut broken,
        ) {
            Ok(bytes) | Err(bytes) => bytes,
        };
        broken.get_or_insert_with(|| Broken::ended(err));
        if !app.reply(bytes).await {
            gone = true;
        }
    }
    let tail = if gone {
        Vec::new()
    } else {
        encoder.finish().unwrap_or_else(|e| {
            tracing::error!(error = %e, "kpp cannot end the stream for the app");
            broken.get_or_insert_with(|| Broken::encoder(e));
            Vec::new()
        })
    };
    app.progress.bytes += tail.len() as u64;
    let estimate = !tally.has_final_usage()
        && (assembled.complete || gone || (broken.is_some() && assembled.response.is_some()));
    let ended = app.arrived.elapsed();
    {
        let o = rec.outcome();
        if let Some(r) = &assembled.response {
            o.usage = Some(r.usage.clone());
            o.response_model = Some(r.model.clone());
        }
        o.reply = assembled.response;
        o.stream_progress = Some(app.progress);
    }
    let reported = rec.outcome().usage.clone().unwrap_or_default();
    let count = estimate && reported.total_input_tokens() == 0;
    let mut to_app = Some(tail);
    if count {
        if let Some(tail) = to_app.take()
            && !tail.is_empty()
        {
            app.deliver(Bytes::from(tail)).await;
        }
        rec.outcome().duration = ended;
    }
    if estimate {
        let input = if count {
            Some(counter.count().await.tokens())
        } else {
            None
        };
        let reason = if gone {
            EstimateReason::Cancelled
        } else if broken.is_some() {
            EstimateReason::Failed
        } else {
            EstimateReason::Unreported
        };
        let o = rec.outcome();
        o.usage = Some(estimate::complete(&reported, &tally, input));
        o.usage_estimated = true;
        count_estimate(reason, o.provider.unwrap_or_default());
    }
    if gone {
        rec.finish_cancelled().await;
        return;
    }
    if let Some(c) = carry
        && broken.is_none()
    {
        c.keep().await;
    }
    match broken {
        Some(b) => {
            let provider = rec.outcome().provider.unwrap_or_default();
            metrics::counter!(crate::metrics::STREAM_FAILURES, "kind" => b.kind.clone(), "provider" => provider)
                .increment(1);
            rec.fail_as(&b.kind, &b.err, b.status).await;
        }
        None => rec.finish(200).await,
    }
    if let Some(tail) = to_app
        && !tail.is_empty()
    {
        app.deliver(Bytes::from(tail)).await;
    }
}

async fn end_capped(
    app: &mut ToApp,
    encoder: &mut dyn StreamEncoder,
    broken: &mut Option<Broken>,
) -> bool {
    let err =
        KppError::provider("The reply reached this install's limit for a stream; Kaeso ended it.")
            .with_status(502);
    broken.get_or_insert_with(|| Broken::capped(err.clone()));
    let bytes = match encode(encoder, &StreamEvent::Error(err), broken) {
        Ok(bytes) | Err(bytes) => bytes,
    };
    app.reply(bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_turns_keepalives_off() {
        assert!(
            Keepalive::new(Duration::ZERO)
                .deadline(Instant::now())
                .is_none()
        );
        let now = Instant::now();
        assert_eq!(
            Keepalive::new(Duration::from_secs(15)).deadline(now),
            Some(now + Duration::from_secs(15))
        );
    }
}
