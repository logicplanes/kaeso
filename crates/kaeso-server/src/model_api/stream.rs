use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use axum::body::Body;
use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::{StreamDecoder, StreamEncoder};
use kaeso_core::assembler::Assembler;
use kaeso_core::estimate::{self, Tally};
use kaeso_core::provider::AccountSignIn;
use kaeso_kpp::{DialectId, KppError, Reply, RequestKind, StopReason, StreamEvent};
use kaeso_upstream::memory::AnswerMemory;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::{Instant, sleep_until};
use tokio_util::sync::CancellationToken;

use kaeso_upstream::UpstreamError;

use crate::services::log_media::{KEPT_PER_REQUEST, LeftOut, NOT_KEPT, TOO_LARGE};

const INCOMPLETE: &str = "incomplete";

use super::hooks::{CLIENT_STALLED, SHUT_DOWN, STREAM_INTERRUPTED, STREAM_LIMIT, StreamProgress};
use super::input_count::InputCounter;
use super::pipeline::{
    EstimateReason, Recording, count_estimate, count_upstream_error, shutting_down, transport_kind,
    upstream_error,
};
use crate::services::continuity::StreamCarry;

// A streamed image or speech reply as the request log keeps it. Its media is gathered only
// where media is kept and while it fits the request's allowance; otherwise only the start of
// each file is held, which is enough to log its type and size.
struct StreamedMedia {
    logged: bool,
    keep: bool,
    left: u64,
    left_out: LeftOut,
    audio: Vec<u8>,
    audio_bytes: u64,
    audio_type: Option<String>,
    images: Vec<kaeso_kpp::GeneratedImage>,
}

// Enough of a file to read its type from.
const STREAMED_HEAD: usize = 64;

impl StreamedMedia {
    fn new(logged: bool, keep: bool) -> Self {
        StreamedMedia {
            logged,
            keep,
            left: KEPT_PER_REQUEST,
            left_out: LeftOut::default(),
            audio: Vec::new(),
            audio_bytes: 0,
            audio_type: None,
            images: Vec::new(),
        }
    }

    fn reason(&self) -> &'static str {
        if self.keep { TOO_LARGE } else { NOT_KEPT }
    }

    fn feed(&mut self, event: &StreamEvent) {
        use base64::Engine as _;
        if !self.logged {
            return;
        }
        match event {
            StreamEvent::Speech(e) => {
                if self.audio_type.is_none() {
                    self.audio_type.clone_from(&e.media_type);
                }
                let piece = e.audio.as_deref().unwrap_or_default();
                let size = (piece.trim_end_matches('=').len() as u64).saturating_mul(3) / 4;
                let held = self.audio_bytes;
                self.audio_bytes = held.saturating_add(size);
                let fits = self.keep && self.audio_bytes <= self.left;
                if fits || self.audio.len() < STREAMED_HEAD {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(piece)
                        .unwrap_or_default();
                    self.audio.extend_from_slice(&bytes);
                }
                if !fits {
                    self.audio.truncate(STREAMED_HEAD);
                }
            }
            StreamEvent::Image(i) if i.partial.is_none() => {
                let mut image = i.image.clone();
                if let kaeso_kpp::Source::Base64(b) = &image.source {
                    let size = b.data.len() as u64;
                    match self.left.checked_sub(size).filter(|_| self.keep) {
                        Some(rest) => self.left = rest,
                        None => self.left_out.take(&mut image.source, size, self.reason()),
                    }
                }
                self.images.push(image);
            }
            _ => {}
        }
    }

    fn reply(mut self, model: &str, content_type: Option<&str>) -> Option<String> {
        let reply = if !self.images.is_empty() {
            Reply::Image(kaeso_kpp::ImageResponse::new(model, self.images))
        } else if self.audio_bytes > 0 {
            let media_type = self
                .audio_type
                .as_deref()
                .or(content_type)
                .unwrap_or("application/octet-stream")
                .to_owned();
            let whole = self.keep && self.audio_bytes <= self.left;
            let reason = self.reason();
            let mut audio = kaeso_kpp::Source::bytes(media_type, self.audio);
            if !whole {
                self.left_out.take(&mut audio, self.audio_bytes, reason);
            }
            Reply::Speech(kaeso_kpp::SpeechResponse::new(model, audio))
        } else {
            return None;
        };
        crate::services::logs::written_media_reply(&reply, &self.left_out)
            .map_err(|e| tracing::warn!(error = %e, "cannot serialize a streamed reply for the request log; it is logged without it"))
            .ok()
    }
}

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
            UpstreamError::Path => Broken {
                kind: err.kind.as_str().to_owned(),
                status: 400,
                err,
            },
            UpstreamError::Busy => Broken {
                kind: err.kind.as_str().to_owned(),
                status: 503,
                err,
            },
            UpstreamError::Connect(_)
            | UpstreamError::Transfer(_)
            | UpstreamError::TooLarge(_)
            | UpstreamError::Client(_)
            | UpstreamError::Method(_)
            | UpstreamError::Refused(_) => Broken {
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

// The bytes the response body handed to the connection: what is still queued in the channel
// when the app leaves never reached it, so `StreamProgress::bytes` can't tell a finished reply.
#[derive(Clone, Debug, Default)]
pub(super) struct Taken(Arc<AtomicU64>);

impl Taken {
    fn add(&self, n: usize) {
        self.0
            .fetch_add(u64::try_from(n).unwrap_or(u64::MAX), Ordering::Release);
    }

    fn reached(&self, bytes: u64) -> bool {
        self.0.load(Ordering::Acquire) >= bytes
    }
}

// A binary reply (audio, not events) has no way to carry an error in the body, so a provider that
// fails after its first bytes ends the response as an error instead of as a clean, short file.
#[derive(Clone, Debug, Default)]
pub(super) struct Aborted(Arc<AtomicBool>);

impl Aborted {
    fn set(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn is_set(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub(super) fn body(rx: Receiver<Bytes>, taken: Taken, aborted: Aborted) -> Body {
    Body::from_stream(futures::stream::unfold(
        Some((rx, taken, aborted)),
        |state| async move {
            let (mut rx, taken, aborted) = state?;
            match rx.recv().await {
                Some(bytes) => {
                    taken.add(bytes.len());
                    Some((Ok(bytes), Some((rx, taken, aborted))))
                }
                None if aborted.is_set() => Some((
                    Err(std::io::Error::other("the provider's reply broke off")),
                    None,
                )),
                None => None,
            }
        },
    ))
}

// A binary reply's handler waits for its first bytes (`None`) before it answers, or for the error
// that came before any, which it answers as an error in the app's format.
pub(super) struct Binary {
    pub(super) first: tokio::sync::oneshot::Sender<Option<KppError>>,
    pub(super) aborted: Aborted,
}

struct ToApp {
    tx: Sender<Bytes>,
    taken: Taken,
    progress: StreamProgress,
    arrived: std::time::Instant,
    cut: CancellationToken,
    patience: Duration,
    stopping: bool,
    stalled: bool,
    cut_short: bool,
}

impl ToApp {
    async fn deliver(&mut self, bytes: Bytes) -> bool {
        let patience = if self.stopping {
            SHUTDOWN_WRITE
        } else {
            self.patience
        };
        // Most chunks fit in the channel at once; only a full one needs the timer and the cut.
        let bytes = match self.tx.try_send(bytes) {
            Ok(()) => return true,
            Err(TrySendError::Closed(_)) => return false,
            Err(TrySendError::Full(bytes)) => bytes,
        };
        tokio::select! {
            sent = self.tx.send(bytes) => sent.is_ok(),
            () = tokio::time::sleep(patience) => {
                tracing::info!(waited = ?patience, "the app stopped reading a stream; ending it");
                self.stalled = !self.stopping;
                false
            }
            () = self.cut.cancelled(), if !self.stopping => {
                self.cut_short = true;
                false
            }
        }
    }

    // A write that failed because the gateway is shutting down, not because the app left: only a
    // closed channel means the app is gone; one that merely stopped reading still gets a shutdown.
    fn left(&self) -> bool {
        !self.cut_short || self.tx.is_closed()
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

// The keepalive an encoder writes in place of an event the app's format can't show or that it
// holds back, which the request log counts like the ones the interval earns. The app's own
// format's `ping` is the provider's event, replayed.
fn stands_in(frame: &[u8], bytes: &[u8], event: &StreamEvent, app: DialectId) -> bool {
    !frame.is_empty()
        && bytes == frame
        && !matches!(event, StreamEvent::Extra(item) if item.origin.is(app))
}

#[derive(Default)]
struct Processing(Duration);

impl Processing {
    fn time<T>(&mut self, work: impl FnOnce() -> T) -> T {
        let started = std::time::Instant::now();
        let done = work();
        self.0 += started.elapsed();
        done
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
    pub(super) app: DialectId,
    pub(super) keepalive: Keepalive,
    pub(super) counter: InputCounter,
    pub(super) carry: Option<StreamCarry>,
    pub(super) keep_reply: bool,
    pub(super) account: Option<AccountSignIn>,
    pub(super) taken: Taken,
    pub(super) binary: Option<Binary>,
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

    fn keep(&mut self, rec: &Recording) {
        let Some(due) = self.due else { return };
        let now = rec.state().clock.now();
        if now < due {
            return;
        }
        self.due = Some(now + RESERVATION_REFRESH);
        // The chunk that is due goes on to the app while Valkey answers; an extend that lands
        // after the charge finds no reservation left to move.
        let (state, reservation) = (rec.state().clone(), rec.reservation().clone());
        let request_id = self.request_id.clone();
        rec.state().in_flight.spawn(async move {
            if let Err(e) = crate::services::limits::extend(&state, &reservation, &request_id).await
            {
                metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "reservation")
                    .increment(1);
                tracing::warn!(error = %e, "cannot extend a stream's reservation under its limits");
            }
        });
    }
}

pub(super) async fn pump(parts: Parts, tx: Sender<Bytes>, mut rec: Recording) {
    let state = rec.state().clone();
    let mut reserved = Reserved::new(&mut rec);
    let Parts {
        mut chunks,
        mut decoder,
        mut encoder,
        app: app_dialect,
        keepalive,
        counter,
        mut carry,
        keep_reply,
        account,
        taken,
        binary,
    } = parts;
    let (mut first, aborted) = match binary {
        Some(b) => (Some(b.first), Some(b.aborted)),
        None => (None, None),
    };
    let mut asm = if keep_reply {
        Assembler::new(counter.dialect)
    } else {
        Assembler::without_blocks(counter.dialect)
    };
    let mut tally = Tally::new();
    let mut streamed = StreamedMedia::new(rec.outcome().content_kept, rec.outcome().media_kept);
    let mut app = ToApp {
        tx,
        taken,
        progress: StreamProgress::default(),
        arrived: rec.arrived(),
        cut: state.in_flight.cut(),
        patience: state.config.upstream.stream_idle_timeout,
        stopping: false,
        stalled: false,
        cut_short: false,
    };
    let limits = &state.config.streams;
    let max_bytes = limits.max_bytes.0;
    // The reply, its log copy and an encoder that answers with the whole reply at its end grow
    // with every byte the provider sends, so the stream holds the answers' memory for them.
    let mut holding = state.upstream.answers().map(AnswerMemory::charge);
    let mut received = 0usize;
    let mut gone = false;
    let mut broken: Option<Broken> = None;
    let mut last_event_at: Option<u64> = None;
    let mut processing = Processing::default();
    let mut last_write = Instant::now();
    let frame = encoder.keepalive();
    {
        let watch = app.tx.clone();
        let closed = watch.closed();
        tokio::pin!(closed);
        let quiet = sleep_until(keepalive.deadline(last_write).unwrap_or(last_write));
        tokio::pin!(quiet);
        let cut = app.cut.clone();
        let cut = cut.cancelled();
        tokio::pin!(cut);
        let too_long = tokio::time::sleep(limits.max_duration);
        tokio::pin!(too_long);
        'read: loop {
            let next = tokio::select! {
                () = &mut closed => { gone = true; break 'read; }
                () = &mut too_long, if !limits.max_duration.is_zero() => {
                    if last_event_at.is_none()
                        && !end_capped(&mut app, encoder.as_mut(), &mut broken).await
                    {
                        gone = true;
                    }
                    break 'read;
                }
                () = &mut cut => {
                    if last_event_at.is_none() {
                        end_shutdown(&mut app, encoder.as_mut(), &mut broken).await;
                    }
                    gone = app.tx.is_closed();
                    break 'read;
                }
                next = chunks.next() => {
                    reserved.keep(&rec);
                    next
                }
                () = &mut quiet, if keepalive.interval.is_some() => {
                    if let Some(due) = keepalive.deadline(last_write)
                        && due > Instant::now()
                    {
                        quiet.as_mut().reset(due);
                        continue;
                    }
                    reserved.keep(&rec);
                    if !app.keepalive(encoder.keepalive()).await {
                        gone = cut_off(&mut app, encoder.as_mut(), &mut broken, last_event_at).await;
                        break 'read;
                    }
                    last_write = Instant::now();
                    if let Some(due) = keepalive.deadline(last_write) {
                        quiet.as_mut().reset(due);
                    }
                    continue;
                }
            };
            if let (Some(Ok(bytes)), Some(h)) = (&next, holding.as_mut()) {
                received = received.saturating_add(bytes.len());
                if last_event_at.is_none() && h.cover_streamed(received).await.is_err() {
                    if !end_capped(&mut app, encoder.as_mut(), &mut broken).await {
                        gone = true;
                    }
                    break 'read;
                }
            }
            let (events, last) = match next {
                Some(Ok(bytes)) => {
                    let mut events = processing.time(|| decoder.feed(&bytes));
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
                Some(Err(e)) if last_event_at.is_some() => {
                    tracing::debug!(error = %e, "the provider's stream broke after its last event");
                    (Vec::new(), true)
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
                    let mut events = processing.time(|| decoder.finish());
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
            let mut ends = false;
            let mut in_place = 0u32;
            for event in &events {
                asm.feed(event);
                tally.feed(event);
                streamed.feed(event);
                if let Some(c) = carry.as_mut() {
                    c.observe(event);
                }
                let bytes = match processing.time(|| encode(encoder.as_mut(), event, &mut broken)) {
                    Ok(bytes) => {
                        ends |= matches!(event, StreamEvent::ResponseStop(_));
                        bytes
                    }
                    Err(bytes) => {
                        stop = true;
                        bytes
                    }
                };
                if stands_in(&frame, &bytes, event, app_dialect) {
                    in_place += 1;
                }
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
                    gone = cut_off(&mut app, encoder.as_mut(), &mut broken, last_event_at).await;
                    break 'read;
                }
                last_write = Instant::now();
                app.progress.keepalives = app.progress.keepalives.saturating_add(in_place);
                if let Some(f) = first.take() {
                    let _ = f.send(None);
                }
            }
            if ends {
                last_event_at.get_or_insert(app.progress.bytes);
            }
            if last_event_at.is_none() && max_bytes > 0 && app.progress.bytes >= max_bytes {
                if !end_capped(&mut app, encoder.as_mut(), &mut broken).await {
                    gone = true;
                }
                break 'read;
            }
            if stop {
                break 'read;
            }
            if last {
                break;
            }
        }
    }
    // An early exit leaves what the decoder held back: the app's stream is over, but the logged
    // reply and the usage estimate still count it.
    let held = processing.time(|| decoder.flush());
    app.progress.count(&held);
    for event in &held {
        asm.feed(event);
        tally.feed(event);
        streamed.feed(event);
    }
    drop(chunks);
    let cancelled = gone && !last_event_at.is_some_and(|at| app.taken.reached(at));
    let mut assembled = asm.finish();
    if !assembled.complete
        && let Some(r) = assembled.response.as_mut()
    {
        r.stop = StopReason::Other(INCOMPLETE.to_owned());
    }
    if !gone && broken.is_none() && !assembled.complete {
        let err = KppError::provider("The provider's stream ended before the reply was complete.")
            .with_status(502);
        let bytes = match processing.time(|| {
            encode(
                encoder.as_mut(),
                &StreamEvent::Error(err.clone()),
                &mut broken,
            )
        }) {
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
    let kind = rec.outcome().kind;
    // Image, speech and transcript streams report usage on their last event, or bill by unit.
    let media = matches!(
        kind,
        RequestKind::Image | RequestKind::Speech | RequestKind::Transcription
    );
    let estimate = !media
        && !tally.has_final_usage()
        && (assembled.complete || gone || (broken.is_some() && assembled.response.is_some()));
    let ended = app.arrived.elapsed();
    {
        let o = rec.outcome();
        if let Some(r) = &assembled.response {
            o.usage = Some(r.usage.clone());
            o.response_model = Some(r.model.clone());
        }
        if let Some(units) = o.media.as_mut() {
            match kind {
                RequestKind::Image => super::media::add_streamed(units, assembled.images),
                RequestKind::Transcription => {
                    if let Some(ms) = assembled.audio_ms {
                        units.input_ms = ms;
                        units.measured = false;
                        units.input_length_unknown = false;
                    }
                }
                RequestKind::Speech => {
                    if let Some(c) = assembled.characters {
                        units.characters = c;
                    }
                }
                _ => {}
            }
        }
        if let Some(r) = o.response_model.as_mut().filter(|m| m.is_empty()) {
            r.clone_from(&o.requested_model.clone().unwrap_or_default());
        }
        o.reply = if media { None } else { assembled.response };
        if media && assembled.complete {
            let model = o.response_model.clone().unwrap_or_default();
            o.media_reply_logged = streamed.reply(&model, o.content_type.as_deref());
        }
        o.stream_progress = Some(app.progress);
        o.stream_processing = Some(processing.0);
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
        let reason = if cancelled {
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
    if cancelled && app.stalled {
        let provider = rec.outcome().provider.unwrap_or_default();
        metrics::counter!(crate::metrics::STREAM_FAILURES, "kind" => CLIENT_STALLED, "provider" => provider)
            .increment(1);
        rec.finish_stalled().await;
        return;
    }
    if cancelled {
        rec.finish_cancelled().await;
        return;
    }
    if let Some(c) = carry
        && broken.is_none()
    {
        c.keep().await;
    }
    if let (Some(b), Some(aborted)) = (&broken, &aborted) {
        match first.take() {
            Some(f) => {
                let _ = f.send(Some(b.err.clone().with_status(b.status)));
            }
            None => aborted.set(),
        }
    }
    if let Some(f) = first.take() {
        let _ = f.send(None);
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

async fn cut_off(
    app: &mut ToApp,
    encoder: &mut dyn StreamEncoder,
    broken: &mut Option<Broken>,
    last_event_at: Option<u64>,
) -> bool {
    if app.left() {
        return true;
    }
    if last_event_at.is_none() {
        end_shutdown(app, encoder, broken).await;
    }
    app.tx.is_closed()
}

async fn end_shutdown(
    app: &mut ToApp,
    encoder: &mut dyn StreamEncoder,
    broken: &mut Option<Broken>,
) -> bool {
    app.stopping = true;
    let err = shutting_down();
    broken.get_or_insert_with(|| Broken::shutdown(err.clone()));
    let bytes = match encode(encoder, &StreamEvent::Error(err), broken) {
        Ok(bytes) | Err(bytes) => bytes,
    };
    app.reply(bytes).await
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

    fn speech(bytes: &[u8]) -> StreamEvent {
        use base64::Engine as _;
        let mut event = kaeso_kpp::SpeechEvent::default();
        event.audio = Some(base64::engine::general_purpose::STANDARD.encode(bytes));
        event.media_type = Some("audio/mpeg".into());
        StreamEvent::Speech(event)
    }

    #[test]
    fn a_streamed_reply_beyond_the_allowance_is_logged_as_too_large_without_being_held() {
        let piece = vec![7u8; 1 << 20];
        let mut over = StreamedMedia::new(true, true);
        for _ in 0..11 {
            over.feed(&speech(&piece));
        }
        assert!(over.audio.len() <= STREAMED_HEAD);
        let logged: serde_json::Value =
            serde_json::from_str(&over.reply("tts-1", None).unwrap()).unwrap();
        assert_eq!(
            logged["speech"]["audio"]["base64"]["data"]["kaeso_media"],
            serde_json::json!({ "media_type": "audio/mpeg", "bytes": 11 << 20, "reason": "too_large" })
        );

        let mut kept = StreamedMedia::new(true, true);
        kept.feed(&speech(b"ID3\x04 a short sound"));
        let logged = kept.reply("tts-1", None).unwrap();
        assert!(
            logged.starts_with(r#"{"format":2,"speech":"#) && !logged.contains("kaeso_media"),
            "{logged}"
        );

        let mut unlogged = StreamedMedia::new(false, false);
        unlogged.feed(&speech(&piece));
        assert!(unlogged.audio.is_empty() && unlogged.reply("tts-1", None).is_none());
    }

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

    fn writer(capacity: usize) -> (ToApp, Receiver<Bytes>, CancellationToken) {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);
        let cut = CancellationToken::new();
        let app = ToApp {
            tx,
            taken: Taken::default(),
            progress: StreamProgress::default(),
            arrived: std::time::Instant::now(),
            cut: cut.clone(),
            patience: Duration::from_secs(300),
            stopping: false,
            stalled: false,
            cut_short: false,
        };
        (app, rx, cut)
    }

    #[tokio::test(start_paused = true)]
    async fn a_shutdown_during_a_write_to_an_app_that_stopped_reading_ends_as_a_shutdown() {
        let (mut app, rx, cut) = writer(1);
        assert!(app.reply(b"data: one\n\n".to_vec()).await);
        let mut encoder = kaeso_codecs::dispatch::stream_encoder(
            DialectId::OpenAiChat,
            &kaeso_codecs::EncodeCtx::new(
                kaeso_kpp::RequestKind::Generate,
                kaeso_kpp::StreamMode::on(false),
            ),
        )
        .unwrap();
        let mut broken = None;
        cut.cancel();
        assert!(!app.reply(b"data: two\n\n".to_vec()).await);
        assert!(app.cut_short && !app.left(), "the app is slow, not gone");
        let gone = cut_off(&mut app, encoder.as_mut(), &mut broken, None).await;
        assert!(!gone);
        assert_eq!(broken.map(|b| b.kind), Some(SHUT_DOWN.to_owned()));
        drop(rx);
        assert!(app.left(), "a closed channel means the app left");
    }

    #[tokio::test(start_paused = true)]
    async fn a_write_an_app_stopped_reading_without_a_shutdown_means_it_left() {
        let (mut app, _rx, _cut) = writer(1);
        assert!(app.reply(b"data: one\n\n".to_vec()).await);
        assert!(!app.reply(b"data: two\n\n".to_vec()).await);
        assert!(app.stalled && app.left());
    }

    #[tokio::test]
    async fn only_what_the_body_handed_on_counts_as_reaching_the_app() {
        use http_body_util::BodyExt;
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let taken = Taken::default();
        let mut body = body(rx, taken.clone(), Aborted::default());
        tx.send(Bytes::from_static(b"data: one\n\n")).await.unwrap();
        tx.send(Bytes::from_static(b"data: [DONE]\n\n"))
            .await
            .unwrap();
        assert!(!taken.reached(1));
        body.frame().await.unwrap().unwrap();
        assert!(taken.reached(11));
        assert!(!taken.reached(25));
        drop(body);
        assert!(
            !taken.reached(25),
            "a queued last event never reached the app"
        );
    }
}
