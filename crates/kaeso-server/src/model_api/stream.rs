use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::Body;
use bytes::Bytes;
use futures::{Stream as _, StreamExt};
use kaeso_codecs::{StreamDecoder, StreamEncoder, StreamPassthrough};
use kaeso_core::assembler::Assembler;
use kaeso_core::estimate::{self, OutputCap, Tally};
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

struct StreamBody {
    rx: Receiver<Bytes>,
    taken: Taken,
    aborted: Aborted,
    done: bool,
}

impl hyper::body::Body for StreamBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        if self.done {
            return Poll::Ready(None);
        }
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(bytes)) => {
                self.taken.add(bytes.len());
                Poll::Ready(Some(Ok(hyper::body::Frame::data(bytes))))
            }
            Poll::Ready(None) => {
                self.done = true;
                if self.aborted.is_set() {
                    Poll::Ready(Some(Err(std::io::Error::other(
                        "the provider's reply broke off",
                    ))))
                } else {
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub(super) fn body(rx: Receiver<Bytes>, taken: Taken, aborted: Aborted) -> Body {
    Body::new(StreamBody {
        rx,
        taken,
        aborted,
        done: false,
    })
}

// A binary reply's handler waits for its first bytes (`None`) before it answers, or for the error
// that came before any, which it answers as an error in the app's format.
pub(super) struct Binary {
    pub(super) first: tokio::sync::oneshot::Sender<Option<KppError>>,
    pub(super) aborted: Aborted,
}

struct ToApp {
    tx: Option<Sender<Bytes>>,
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
    fn is_closed(&self) -> bool {
        self.tx.as_ref().is_none_or(Sender::is_closed)
    }

    fn close(&mut self) {
        self.tx = None;
    }

    async fn deliver(&mut self, bytes: Bytes) -> bool {
        let Some(tx) = self.tx.as_ref() else {
            return false;
        };
        let patience = if self.stopping {
            SHUTDOWN_WRITE
        } else {
            self.patience
        };
        // Most chunks fit in the channel at once; only a full one needs the timer and the cut.
        let bytes = match tx.try_send(bytes) {
            Ok(()) => return true,
            Err(TrySendError::Closed(_)) => return false,
            Err(TrySendError::Full(bytes)) => bytes,
        };
        tokio::select! {
            sent = tx.send(bytes) => sent.is_ok(),
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
        !self.cut_short || self.is_closed()
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

/// Events already decoded while peeking the start of a stream, passed on to the pump so the
/// first chunks are not decoded twice.
pub(super) struct Initial {
    pub(super) events: Vec<StreamEvent>,
    pub(super) received: usize,
    pub(super) error: Option<UpstreamError>,
}

/// Raw bytes already scanned while peeking a same-dialect passthrough stream.
pub(super) struct PassthroughInitial {
    pub(super) bytes: Vec<u8>,
    pub(super) events: u32,
    pub(super) ended: bool,
    pub(super) eof: bool,
    pub(super) received: usize,
    pub(super) error: Option<UpstreamError>,
    pub(super) provider_error: Option<KppError>,
}

pub(super) enum Peeked {
    Decoded {
        decoder: Box<dyn StreamDecoder>,
        initial: Initial,
    },
    PutBack,
}

pub(super) enum Mode {
    Codec {
        decoder: Box<dyn StreamDecoder>,
        encoder: Box<dyn StreamEncoder>,
        initial: Option<Initial>,
    },
    Passthrough {
        passthrough: Box<dyn StreamPassthrough>,
        initial: Option<PassthroughInitial>,
    },
}

pub(super) struct Parts {
    pub(super) chunks: kaeso_upstream::ByteStream,
    pub(super) mode: Mode,
    pub(super) app: DialectId,
    pub(super) keepalive: Keepalive,
    pub(super) counter: InputCounter,
    pub(super) carry: Option<StreamCarry>,
    pub(super) keep_reply: bool,
    pub(super) account: Option<AccountSignIn>,
    pub(super) taken: Taken,
    pub(super) binary: Option<Binary>,
    // Puts back the values built-in rules masked, where the model names their placeholders.
    pub(super) restoring: Option<crate::services::guardrails::rules::Restoring>,
    pub(super) output_cap: Option<OutputCap>,
}

pub(super) enum Gathered {
    Transport(UpstreamError),
    Provider(KppError),
    Broken(KppError),
}

// Reads a provider's whole stream before anything goes to the app, so a guardrail can check
// the reply first.
pub(super) async fn hold(
    mut chunks: kaeso_upstream::ByteStream,
    max: usize,
) -> Result<Vec<Bytes>, UpstreamError> {
    let mut held = Vec::new();
    let mut read = 0usize;
    while let Some(chunk) = chunks.next().await {
        let bytes = chunk?;
        read = read.saturating_add(bytes.len());
        if read > max {
            return Err(UpstreamError::TooLarge(max));
        }
        held.push(bytes);
    }
    Ok(held)
}

/// The most of an event stream that is read before the reply starts: the events before the
/// first output are small, such as a response start and pings.
const PEEK_MOST: usize = 64 * 1024;

async fn poll_next_now(
    chunks: &mut kaeso_upstream::ByteStream,
) -> Poll<Option<Result<Bytes, UpstreamError>>> {
    std::future::poll_fn(|cx| Poll::Ready(Pin::new(&mut *chunks).poll_next(cx))).await
}

fn extend_events(dst: &mut Vec<StreamEvent>, src: Vec<StreamEvent>) {
    if dst.is_empty() {
        *dst = src;
    } else {
        dst.extend(src);
    }
}

/// Reads a provider's event stream until its first event that carries output, and returns the
/// provider's error when one comes before it: such a stream fails the attempt like an error
/// status would, and an alias route can fall back. It stops waiting when the first keepalive
/// would be due, and puts back what it read; a broken connection stays the pump's to report.
pub(super) async fn peek(
    chunks: &mut kaeso_upstream::ByteStream,
    mut decoder: Box<dyn StreamDecoder>,
    wait: Duration,
) -> Option<KppError> {
    let sleep = sleep_until(Instant::now() + wait);
    tokio::pin!(sleep);
    let mut read = Vec::new();
    let mut size = 0usize;
    while size < PEEK_MOST {
        let next = match poll_next_now(chunks).await {
            Poll::Ready(item) => item,
            Poll::Pending => {
                tokio::select! {
                    biased;
                    next = chunks.next() => next,
                    () = &mut sleep => break,
                }
            }
        };
        let (events, ended) = match next {
            Some(Ok(bytes)) => {
                size = size.saturating_add(bytes.len());
                let events = decoder.feed(&bytes);
                read.push(bytes);
                (events, false)
            }
            Some(Err(e)) => {
                chunks.put_back(read, Some(e));
                return None;
            }
            // A decoder may hold an error back to pair it with a later event, such as a
            // Responses `error` before `response.failed`; `finish` would add its own error for
            // the cut-off reply, which stays the pump's to report.
            None => (decoder.flush(), true),
        };
        match first_of(&events) {
            Seen::Error(err) => return Some(err),
            Seen::Output => break,
            Seen::Nothing if ended => break,
            Seen::Nothing => {}
        }
    }
    chunks.put_back(read, None);
    None
}

/// Like [`peek`], but hands the live decoder and already-decoded events on to the pump instead of
/// putting the raw bytes back to be decoded a second time.
pub(super) async fn peek_keep(
    chunks: &mut kaeso_upstream::ByteStream,
    mut decoder: Box<dyn StreamDecoder>,
    wait: Duration,
) -> Result<Peeked, KppError> {
    let sleep = sleep_until(Instant::now() + wait);
    tokio::pin!(sleep);
    let mut read = Vec::new();
    let mut events = Vec::new();
    let mut received = 0usize;
    let mut error = None;
    while received < PEEK_MOST {
        let next = match poll_next_now(chunks).await {
            Poll::Ready(item) => item,
            Poll::Pending => {
                tokio::select! {
                    biased;
                    next = chunks.next() => next,
                    () = &mut sleep => break,
                }
            }
        };
        match next {
            Some(Ok(bytes)) => {
                received = received.saturating_add(bytes.len());
                let batch = decoder.feed(&bytes);
                read.push(bytes);
                match first_of(&batch) {
                    Seen::Error(err) => return Err(err),
                    Seen::Output => {
                        extend_events(&mut events, batch);
                        break;
                    }
                    Seen::Nothing => extend_events(&mut events, batch),
                }
            }
            Some(Err(e)) => {
                error = Some(e);
                break;
            }
            // `flush` marks some decoders inert, so an empty or cut-off stream with no error is
            // put back for a fresh decoder whose `finish` reports it as the pump expects.
            None => {
                let flushed = decoder.flush();
                if let Seen::Error(err) = first_of(&flushed) {
                    return Err(err);
                }
                chunks.put_back(read, None);
                return Ok(Peeked::PutBack);
            }
        }
    }
    Ok(Peeked::Decoded {
        decoder,
        initial: Initial {
            events,
            received,
            error,
        },
    })
}

/// Peeks a same-dialect passthrough stream until output arrives, failing the attempt if the
/// provider sends an error event before any output.
pub(super) async fn peek_passthrough(
    chunks: &mut kaeso_upstream::ByteStream,
    mut passthrough: Box<dyn StreamPassthrough>,
    wait: Duration,
) -> Result<(Box<dyn StreamPassthrough>, PassthroughInitial), KppError> {
    let sleep = sleep_until(Instant::now() + wait);
    tokio::pin!(sleep);
    let mut out = Vec::new();
    let mut events = 0u32;
    let mut ended = false;
    let mut eof = false;
    let mut received = 0usize;
    let mut saw_output = false;
    let mut error = None;
    let mut provider_error = None;
    while received < PEEK_MOST {
        let next = match poll_next_now(chunks).await {
            Poll::Ready(item) => item,
            Poll::Pending => {
                tokio::select! {
                    biased;
                    next = chunks.next() => next,
                    () = &mut sleep => break,
                }
            }
        };
        match next {
            Some(Ok(bytes)) => {
                received = received.saturating_add(bytes.len());
                let step = passthrough.feed(&bytes, &mut out);
                events = events.saturating_add(step.events);
                ended |= step.ended;
                saw_output |= step.saw_output;
                if let Some(err) = step.error {
                    if !saw_output {
                        return Err(err);
                    }
                    provider_error = Some(err);
                    break;
                }
                if saw_output || ended {
                    break;
                }
            }
            Some(Err(e)) => {
                error = Some(e);
                break;
            }
            None => {
                let step = passthrough.flush_eof_error(&mut out);
                events = events.saturating_add(step.events);
                ended |= step.ended;
                saw_output |= step.saw_output;
                if let Some(err) = step.error {
                    if !saw_output {
                        return Err(err);
                    }
                    provider_error = Some(err);
                }
                eof = true;
                break;
            }
        }
    }
    Ok((
        passthrough,
        PassthroughInitial {
            bytes: out,
            events,
            ended,
            eof,
            received,
            error,
            provider_error,
        },
    ))
}

enum Seen {
    Error(KppError),
    Output,
    Nothing,
}

fn first_of(events: &[StreamEvent]) -> Seen {
    for event in events {
        match event {
            StreamEvent::Error(err) => return Seen::Error(err.clone()),
            StreamEvent::ResponseStart(_) | StreamEvent::Extra(_) => {}
            _ => return Seen::Output,
        }
    }
    Seen::Nothing
}

// A reply Kaeso ends itself is billed for the output the provider wrote, also past the cut;
// whether the provider's usage came is read from the reply, which `OutputCap` keeps it on.
fn count_read(tally: &mut Tally, events: &[StreamEvent]) {
    for event in events.iter().filter(|e| is_output(e)) {
        tally.feed(event);
    }
}

fn is_output(event: &StreamEvent) -> bool {
    matches!(event, StreamEvent::BlockDelta(_))
}

pub(super) async fn gather(
    mut chunks: kaeso_upstream::ByteStream,
    mut decoder: Box<dyn StreamDecoder>,
    dialect: DialectId,
    max: usize,
    mut cap: Option<OutputCap>,
) -> Result<(Reply, Tally), Gathered> {
    let mut asm = Assembler::new(dialect);
    let mut tally = Tally::new();
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
        count_read(&mut tally, &events);
        let (events, capped) = match cap.as_mut() {
            Some(c) => (c.apply(events), c.reached()),
            None => (events, false),
        };
        for event in &events {
            asm.feed(event);
            if !is_output(event) {
                tally.feed(event);
            }
        }
        if last || capped {
            break;
        }
    }
    let assembled = asm.finish();
    if let Some(mut err) = assembled.error {
        super::pipeline::redact_key_fragments(&mut err);
        return Err(Gathered::Provider(err));
    }
    match assembled.response {
        Some(response) if assembled.complete => Ok((Reply::Generate(response), tally)),
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

// Cap how many already-buffered bytes are coalesced into one downstream write so slow clients
// and large bursts still stream smoothly without extra latency on slow providers.
const COALESCE_LIMIT: usize = 32 * 1024;
// Hyper's HTTP/1.1 client body channel holds one decoded chunk at a time; yielding once after
// the first chunk lets its connection task drain already-buffered TCP bytes without adding TTFT
// delay or waiting on slow streams.
const MAX_COALESCE_YIELDS: usize = 64;

struct PendingOut {
    bytes: Vec<u8>,
    ends: bool,
}

enum BatchStart {
    Initial(Initial),
    PassthroughInitial(PassthroughInitial),
    Chunk(Option<Result<Bytes, UpstreamError>>),
}

enum StreamEngine {
    Codec {
        decoder: Box<dyn StreamDecoder>,
        encoder: Box<dyn StreamEncoder>,
        asm: Assembler,
        streamed: StreamedMedia,
    },
    Passthrough(Box<dyn StreamPassthrough>),
}

impl StreamEngine {
    fn encode_error(&mut self, err: &KppError, broken: &mut Option<Broken>) -> Vec<u8> {
        match self {
            StreamEngine::Codec { encoder, .. } => {
                match encode(encoder.as_mut(), &StreamEvent::Error(err.clone()), broken) {
                    Ok(bytes) | Err(bytes) => bytes,
                }
            }
            StreamEngine::Passthrough(p) => p.encode_error(err),
        }
    }

    fn keepalive(&mut self) -> Vec<u8> {
        match self {
            StreamEngine::Codec { encoder, .. } => encoder.keepalive(),
            StreamEngine::Passthrough(p) => p.keepalive(),
        }
    }

    fn finish_tail(&mut self, broken: &mut Option<Broken>) -> Vec<u8> {
        match self {
            StreamEngine::Codec { encoder, .. } => encoder.finish().unwrap_or_else(|e| {
                tracing::error!(error = %e, "kpp cannot end the stream for the app");
                broken.get_or_insert_with(|| Broken::encoder(e));
                Vec::new()
            }),
            StreamEngine::Passthrough(_) => Vec::new(),
        }
    }

    fn has_final_usage(&self, tally: &Tally) -> bool {
        match self {
            StreamEngine::Codec { .. } => tally.has_final_usage(),
            StreamEngine::Passthrough(p) => p.summary().tally.final_usage,
        }
    }
}

trait ErrorEncoder {
    fn encode_stream_error(&mut self, err: &KppError, broken: &mut Option<Broken>) -> Vec<u8>;
}

impl ErrorEncoder for dyn StreamEncoder {
    fn encode_stream_error(&mut self, err: &KppError, broken: &mut Option<Broken>) -> Vec<u8> {
        match encode(self, &StreamEvent::Error(err.clone()), broken) {
            Ok(bytes) | Err(bytes) => bytes,
        }
    }
}

impl ErrorEncoder for StreamEngine {
    fn encode_stream_error(&mut self, err: &KppError, broken: &mut Option<Broken>) -> Vec<u8> {
        self.encode_error(err, broken)
    }
}

struct Pump {
    rec: Recording,
    reserved: Reserved,
    chunks: Option<kaeso_upstream::ByteStream>,
    engine: StreamEngine,
    app_dialect: DialectId,
    keepalive: Keepalive,
    counter: InputCounter,
    carry: Option<StreamCarry>,
    account: Option<AccountSignIn>,
    first: Option<tokio::sync::oneshot::Sender<Option<KppError>>>,
    aborted: Option<Aborted>,
    restoring: Option<crate::services::guardrails::rules::Restoring>,
    output_cap: Option<OutputCap>,
    tally: Tally,
    app: ToApp,
    max_bytes: u64,
    max_duration: Duration,
    holding: Option<kaeso_upstream::memory::Charge>,
    received: usize,
    gone: bool,
    broken: Option<Broken>,
    last_event_at: Option<u64>,
    processing: Processing,
    last_write: Instant,
    frame: Vec<u8>,
    done: bool,
    initial: Option<Initial>,
    passthrough_initial: Option<PassthroughInitial>,
}

impl Pump {
    fn new(parts: Parts, tx: Sender<Bytes>, mut rec: Recording) -> Pump {
        let state = rec.state().clone();
        let reserved = Reserved::new(&mut rec);
        let Parts {
            chunks,
            mode,
            app: app_dialect,
            keepalive,
            counter,
            carry,
            keep_reply,
            account,
            taken,
            binary,
            restoring,
            output_cap,
        } = parts;
        let (first, aborted) = match binary {
            Some(b) => (Some(b.first), Some(b.aborted)),
            None => (None, None),
        };
        let (mut engine, initial, passthrough_initial) = match mode {
            Mode::Codec {
                decoder,
                encoder,
                initial,
            } => {
                let asm = if keep_reply {
                    Assembler::new(counter.dialect)
                } else {
                    Assembler::without_blocks(counter.dialect)
                };
                let streamed =
                    StreamedMedia::new(rec.outcome().content_kept, rec.outcome().media_kept);
                (
                    StreamEngine::Codec {
                        decoder,
                        encoder,
                        asm,
                        streamed,
                    },
                    initial,
                    None,
                )
            }
            Mode::Passthrough {
                passthrough,
                initial,
            } => (StreamEngine::Passthrough(passthrough), None, initial),
        };
        let app = ToApp {
            tx: Some(tx),
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
        let max_duration = limits.max_duration;
        // The reply, its log copy and an encoder that answers with the whole reply at its end grow
        // with every byte the provider sends, so the stream holds the answers' memory for them.
        let holding = state.upstream.answers().map(AnswerMemory::charge);
        let frame = engine.keepalive();
        Pump {
            rec,
            reserved,
            chunks: Some(chunks),
            engine,
            app_dialect,
            keepalive,
            counter,
            carry,
            account,
            first,
            aborted,
            restoring,
            output_cap,
            tally: Tally::new(),
            app,
            max_bytes,
            max_duration,
            holding,
            received: 0,
            gone: false,
            broken: None,
            last_event_at: None,
            processing: Processing::default(),
            last_write: Instant::now(),
            frame,
            done: false,
            initial,
            passthrough_initial,
        }
    }

    fn record_event_errors(&mut self, events: &mut [StreamEvent]) -> bool {
        let mut failed = false;
        for e in events {
            if let StreamEvent::Error(err) = e {
                super::pipeline::redact_key_fragments(err);
                if let Some(limited) = super::accounts::plan_limit(self.account, err) {
                    *err = limited;
                }
                self.broken.get_or_insert_with(|| Broken::event(err));
                failed = true;
            }
        }
        failed
    }

    fn process_one_chunk(
        &mut self,
        next: Option<Result<Bytes, UpstreamError>>,
        out: &mut Vec<u8>,
        ends: &mut bool,
        in_place: &mut u32,
    ) -> (bool, bool) {
        match &mut self.engine {
            StreamEngine::Passthrough(pass) => match next {
                Some(Ok(bytes)) => {
                    let step = pass.feed(&bytes, out);
                    self.app.progress.events = self.app.progress.events.saturating_add(step.events);
                    *ends |= step.ended;
                    if let Some(mut err) = step.error {
                        super::pipeline::redact_key_fragments(&mut err);
                        if let Some(limited) = super::accounts::plan_limit(self.account, &err) {
                            err = limited;
                        }
                        self.broken.get_or_insert_with(|| Broken::event(&err));
                        out.extend_from_slice(&pass.encode_error(&err));
                        return (true, false);
                    }
                    (false, false)
                }
                Some(Err(e)) if *ends || self.last_event_at.is_some() => {
                    tracing::debug!(error = %e, "the provider's stream broke after its last event");
                    (true, false)
                }
                Some(Err(e)) => {
                    count_upstream_error(
                        transport_kind(&e),
                        self.rec.outcome().provider.unwrap_or_default(),
                    );
                    let err = upstream_error(&e);
                    self.broken
                        .get_or_insert_with(|| Broken::transport(&e, err.clone()));
                    out.extend_from_slice(&pass.encode_error(&err));
                    (true, false)
                }
                None => {
                    let step = pass.finish(out);
                    self.app.progress.events = self.app.progress.events.saturating_add(step.events);
                    *ends |= step.ended;
                    if let Some(mut err) = step.error {
                        super::pipeline::redact_key_fragments(&mut err);
                        self.broken
                            .get_or_insert_with(|| Broken::ended(err.clone()));
                        out.extend_from_slice(&pass.encode_error(&err));
                    }
                    (true, false)
                }
            },
            StreamEngine::Codec { .. } => {
                let (events, chunk_last) = self.decode_chunk(next, *ends);
                let stop = self.encode_events(events, out, ends, in_place);
                (chunk_last || self.output_capped(), stop)
            }
        }
    }

    fn decode_chunk(
        &mut self,
        next: Option<Result<Bytes, UpstreamError>>,
        ended_in_batch: bool,
    ) -> (Vec<StreamEvent>, bool) {
        let StreamEngine::Codec { decoder, .. } = &mut self.engine else {
            return (Vec::new(), true);
        };
        match next {
            Some(Ok(bytes)) => {
                let mut events = decoder.feed(&bytes);
                self.app.progress.count(&events);
                let failed = self.record_event_errors(&mut events);
                (events, failed)
            }
            Some(Err(e)) if ended_in_batch || self.last_event_at.is_some() => {
                tracing::debug!(error = %e, "the provider's stream broke after its last event");
                (Vec::new(), true)
            }
            Some(Err(e)) => {
                count_upstream_error(
                    transport_kind(&e),
                    self.rec.outcome().provider.unwrap_or_default(),
                );
                let err = upstream_error(&e);
                self.broken
                    .get_or_insert_with(|| Broken::transport(&e, err.clone()));
                (vec![StreamEvent::Error(err)], true)
            }
            None => {
                let mut events = decoder.finish();
                self.app.progress.count(&events);
                for e in &mut events {
                    if let StreamEvent::Error(err) = e {
                        super::pipeline::redact_key_fragments(err);
                        self.broken
                            .get_or_insert_with(|| Broken::ended(err.clone()));
                    }
                }
                (events, true)
            }
        }
    }

    fn encode_events(
        &mut self,
        events: Vec<StreamEvent>,
        out: &mut Vec<u8>,
        ends: &mut bool,
        in_place: &mut u32,
    ) -> bool {
        let StreamEngine::Codec {
            encoder,
            asm,
            streamed,
            ..
        } = &mut self.engine
        else {
            return false;
        };
        let events = match self.output_cap.as_mut() {
            Some(c) => {
                count_read(&mut self.tally, &events);
                c.apply(events)
            }
            None => events,
        };
        let events = match self.restoring.as_mut() {
            Some(r) => r.apply(events),
            None => events,
        };
        let mut stop = false;
        for event in &events {
            asm.feed(event);
            if self.output_cap.is_none() || !is_output(event) {
                self.tally.feed(event);
            }
            streamed.feed(event);
            if let Some(c) = self.carry.as_mut() {
                c.observe(event);
            }
            let bytes = match encode(encoder.as_mut(), event, &mut self.broken) {
                Ok(bytes) => {
                    *ends |= matches!(event, StreamEvent::ResponseStop(_));
                    bytes
                }
                Err(bytes) => {
                    stop = true;
                    bytes
                }
            };
            if stands_in(&self.frame, &bytes, event, self.app_dialect) {
                *in_place += 1;
            }
            if out.is_empty() {
                *out = bytes;
            } else {
                out.extend_from_slice(&bytes);
            }
            if stop {
                break;
            }
        }
        stop
    }

    fn output_capped(&self) -> bool {
        self.output_cap.as_ref().is_some_and(OutputCap::reached)
    }

    fn can_finish_before_accounting(&self) -> bool {
        self.broken.is_none()
            && self.first.is_none()
            && !self.rec.reservation().is_held()
            && self.rec.kept.is_none()
            && self.carry.is_none()
    }

    // Only the batch's own work counts as stream processing: decoding, translating and encoding
    // its chunks. Waiting for the answer's memory, for more chunks to coalesce and for the
    // scheduler is time the stream spent waiting, not Kaeso's work.
    async fn process_batch(&mut self, start: BatchStart) -> Option<PendingOut> {
        let mut out = Vec::new();
        let mut stop = false;
        let mut ends = false;
        let mut in_place = 0u32;
        let mut last = false;
        let mut capped = false;
        match start {
            BatchStart::Initial(mut init) => {
                if init.received > 0
                    && let Some(h) = self.holding.as_mut()
                {
                    self.received = self.received.saturating_add(init.received);
                    if self.last_event_at.is_none()
                        && h.cover_streamed(self.received).await.is_err()
                    {
                        capped = true;
                        last = true;
                    }
                }
                if !capped {
                    let timed = std::time::Instant::now();
                    self.app.progress.count(&init.events);
                    last = self.record_event_errors(&mut init.events);
                    stop = self.encode_events(init.events, &mut out, &mut ends, &mut in_place);
                    self.processing.0 += timed.elapsed();
                    last |= self.output_capped();
                    if !stop
                        && !last
                        && let Some(err) = init.error
                    {
                        let timed = std::time::Instant::now();
                        let (chunk_last, chunk_stop) = self.process_one_chunk(
                            Some(Err(err)),
                            &mut out,
                            &mut ends,
                            &mut in_place,
                        );
                        self.processing.0 += timed.elapsed();
                        last = chunk_last;
                        stop = chunk_stop;
                    }
                }
            }
            BatchStart::PassthroughInitial(init) => {
                if init.received > 0
                    && let Some(h) = self.holding.as_mut()
                {
                    self.received = self.received.saturating_add(init.received);
                    if self.last_event_at.is_none()
                        && h.cover_streamed(self.received).await.is_err()
                    {
                        capped = true;
                        last = true;
                    }
                }
                if !capped {
                    out = init.bytes;
                    self.app.progress.events = self.app.progress.events.saturating_add(init.events);
                    ends |= init.ended;
                    if let Some(mut err) = init.provider_error {
                        super::pipeline::redact_key_fragments(&mut err);
                        if let Some(limited) = super::accounts::plan_limit(self.account, &err) {
                            err = limited;
                        }
                        self.broken.get_or_insert_with(|| Broken::event(&err));
                        out.extend_from_slice(&self.engine.encode_error(&err, &mut self.broken));
                        last = true;
                    } else if let Some(err) = init.error {
                        let timed = std::time::Instant::now();
                        let (chunk_last, chunk_stop) = self.process_one_chunk(
                            Some(Err(err)),
                            &mut out,
                            &mut ends,
                            &mut in_place,
                        );
                        self.processing.0 += timed.elapsed();
                        last = chunk_last;
                        stop = chunk_stop;
                    } else if init.eof {
                        let timed = std::time::Instant::now();
                        let (chunk_last, chunk_stop) =
                            self.process_one_chunk(None, &mut out, &mut ends, &mut in_place);
                        self.processing.0 += timed.elapsed();
                        last = chunk_last;
                        stop = chunk_stop;
                    }
                }
            }
            BatchStart::Chunk(next) => {
                if let (Some(Ok(bytes)), Some(h)) = (&next, self.holding.as_mut()) {
                    self.received = self.received.saturating_add(bytes.len());
                    if self.last_event_at.is_none()
                        && h.cover_streamed(self.received).await.is_err()
                    {
                        capped = true;
                        last = true;
                    }
                }
                if !capped {
                    let timed = std::time::Instant::now();
                    let (chunk_last, chunk_stop) =
                        self.process_one_chunk(next, &mut out, &mut ends, &mut in_place);
                    self.processing.0 += timed.elapsed();
                    last = chunk_last;
                    stop = chunk_stop;
                }
            }
        }
        // Drain any chunks already buffered in the upstream connection into the same write so
        // fast responses don't pay a channel send and HTTP chunk frame per SSE event.
        let mut yields = 0usize;
        while !last
            && !stop
            && !capped
            && out.len() < COALESCE_LIMIT
            && (self.max_bytes == 0
                || self.app.progress.bytes.saturating_add(out.len() as u64) < self.max_bytes)
            && !self.app.cut.is_cancelled()
        {
            let Some(chunks) = self.chunks.as_mut() else {
                break;
            };
            let next = match poll_next_now(chunks).await {
                Poll::Ready(next) => next,
                Poll::Pending if self.app.progress.bytes > 0 && yields < MAX_COALESCE_YIELDS => {
                    yields += 1;
                    tokio::task::yield_now().await;
                    let Some(chunks) = self.chunks.as_mut() else {
                        break;
                    };
                    let Poll::Ready(next) = poll_next_now(chunks).await else {
                        break;
                    };
                    next
                }
                Poll::Pending => break,
            };
            if let (Some(Ok(bytes)), Some(h)) = (&next, self.holding.as_mut()) {
                self.received = self.received.saturating_add(bytes.len());
                if !ends
                    && self.last_event_at.is_none()
                    && h.cover_streamed(self.received).await.is_err()
                {
                    capped = true;
                    last = true;
                    break;
                }
            }
            let timed = std::time::Instant::now();
            let (chunk_last, chunk_stop) =
                self.process_one_chunk(next, &mut out, &mut ends, &mut in_place);
            self.processing.0 += timed.elapsed();
            last = chunk_last;
            stop = chunk_stop;
        }
        if last
            && !stop
            && !capped
            && self.can_finish_before_accounting()
            && (ends
                || self.last_event_at.is_some()
                || self.max_bytes == 0
                || self.app.progress.bytes.saturating_add(out.len() as u64) < self.max_bytes)
        {
            self.app.progress.keepalives = self.app.progress.keepalives.saturating_add(in_place);
            self.done = true;
            return Some(PendingOut { bytes: out, ends });
        }
        // Kept before the app gets the stream's end, after which its next step may come at once.
        if ends
            && self.broken.is_none()
            && let Some(c) = self.carry.take()
        {
            c.keep().await;
        }
        if !out.is_empty() {
            if !self.app.reply(out).await {
                self.gone = cut_off(
                    &mut self.app,
                    &mut self.engine,
                    &mut self.broken,
                    self.last_event_at,
                )
                .await;
                self.done = true;
                return None;
            }
            self.last_write = Instant::now();
            self.app.progress.keepalives = self.app.progress.keepalives.saturating_add(in_place);
            if let Some(f) = self.first.take() {
                let _ = f.send(None);
            }
        }
        if ends {
            self.last_event_at.get_or_insert(self.app.progress.bytes);
        }
        if capped
            || (self.last_event_at.is_none()
                && self.max_bytes > 0
                && self.app.progress.bytes >= self.max_bytes)
        {
            if !end_capped(&mut self.app, &mut self.engine, &mut self.broken).await {
                self.gone = true;
            }
            self.done = true;
            return None;
        }
        if stop || last {
            self.done = true;
        }
        None
    }

    async fn run(mut self, mut pending: Option<PendingOut>) {
        if !self.done {
            if let Some(init) = self.passthrough_initial.take() {
                pending = self
                    .process_batch(BatchStart::PassthroughInitial(init))
                    .await;
            } else if let Some(init) = self.initial.take() {
                pending = self.process_batch(BatchStart::Initial(init)).await;
            }
        }
        if !self.done {
            let Some(watch) = self.app.tx.clone() else {
                self.gone = true;
                Box::pin(self.finalize(pending)).await;
                return;
            };
            let closed = watch.closed();
            tokio::pin!(closed);
            let quiet = sleep_until(
                self.keepalive
                    .deadline(self.last_write)
                    .unwrap_or(self.last_write),
            );
            tokio::pin!(quiet);
            let cut = self.app.cut.clone();
            let cut = cut.cancelled();
            tokio::pin!(cut);
            let too_long = tokio::time::sleep(self.max_duration);
            tokio::pin!(too_long);
            'read: loop {
                let next = tokio::select! {
                    () = &mut closed => { self.gone = true; break 'read; }
                    () = &mut too_long, if !self.max_duration.is_zero() => {
                        if self.last_event_at.is_none()
                            && !end_capped(&mut self.app, &mut self.engine, &mut self.broken).await
                        {
                            self.gone = true;
                        }
                        break 'read;
                    }
                    () = &mut cut => {
                        if self.last_event_at.is_none() {
                            end_shutdown(&mut self.app, &mut self.engine, &mut self.broken).await;
                        }
                        self.gone = self.app.is_closed();
                        break 'read;
                    }
                    next = async {
                        match self.chunks.as_mut() {
                            Some(chunks) => chunks.next().await,
                            None => None,
                        }
                    } => {
                        self.reserved.keep(&self.rec);
                        next
                    }
                    () = &mut quiet, if self.keepalive.interval.is_some() => {
                        if let Some(due) = self.keepalive.deadline(self.last_write)
                            && due > Instant::now()
                        {
                            quiet.as_mut().reset(due);
                            continue;
                        }
                        self.reserved.keep(&self.rec);
                        let ka = self.engine.keepalive();
                        if !self.app.keepalive(ka).await {
                            self.gone = cut_off(
                                &mut self.app,
                                &mut self.engine,
                                &mut self.broken,
                                self.last_event_at,
                            )
                            .await;
                            break 'read;
                        }
                        self.last_write = Instant::now();
                        if let Some(due) = self.keepalive.deadline(self.last_write) {
                            quiet.as_mut().reset(due);
                        }
                        continue;
                    }
                };
                pending = self.process_batch(BatchStart::Chunk(next)).await;
                if self.done {
                    break 'read;
                }
            }
        }
        // Boxed so an open stream's task holds a pointer, not the state of the work that ends
        // it: that state is several times the size of everything the stream needs while open.
        Box::pin(self.finalize(pending)).await;
    }

    async fn finalize(mut self, pending: Option<PendingOut>) {
        self.chunks = None;
        let mut assembled = match &mut self.engine {
            StreamEngine::Codec {
                decoder,
                asm,
                streamed,
                ..
            } => {
                // An early exit leaves what the decoder held back: the app's stream is over, but
                // the logged reply and the usage estimate still count it.
                let held = self.processing.time(|| decoder.flush());
                let held = match self.output_cap.as_mut() {
                    Some(c) => {
                        count_read(&mut self.tally, &held);
                        c.apply(held)
                    }
                    None => held,
                };
                self.app.progress.count(&held);
                for event in &held {
                    asm.feed(event);
                    if self.output_cap.is_none() || !is_output(event) {
                        self.tally.feed(event);
                    }
                    streamed.feed(event);
                }
                std::mem::replace(asm, Assembler::without_blocks(self.counter.dialect)).finish()
            }
            StreamEngine::Passthrough(pass) => {
                let summary = pass.summary();
                self.tally = Tally::from_counts(
                    summary.tally.deltas,
                    summary.tally.bytes,
                    summary.tally.reasoning_deltas,
                    summary.tally.reasoning_bytes,
                    summary.tally.final_usage,
                );
                let response = summary.started.then(|| {
                    let mut r = kaeso_kpp::Response::new(summary.id, summary.model, Vec::new());
                    r.stop = summary.stop;
                    r.usage = summary.usage;
                    r
                });
                kaeso_core::assembler::Assembled {
                    response,
                    complete: summary.complete,
                    error: None,
                    images: 0,
                    audio_bytes: 0,
                    audio_ms: None,
                    characters: None,
                }
            }
        };
        if !assembled.complete
            && let Some(r) = assembled.response.as_mut()
        {
            r.stop = StopReason::Other(INCOMPLETE.to_owned());
        }
        let mut fused = pending;
        if !self.gone && self.broken.is_none() && !assembled.complete {
            let err =
                KppError::provider("The provider's stream ended before the reply was complete.")
                    .with_status(502);
            let bytes = self
                .processing
                .time(|| self.engine.encode_error(&err, &mut self.broken));
            self.broken.get_or_insert_with(|| Broken::ended(err));
            if let Some(p) = fused.take()
                && !p.bytes.is_empty()
                && !self.app.reply(p.bytes).await
            {
                self.gone = true;
            }
            if !self.gone && !self.app.reply(bytes).await {
                self.gone = true;
            }
        }
        let tail = if self.gone {
            Vec::new()
        } else {
            self.engine.finish_tail(&mut self.broken)
        };
        let mut to_app = Some(tail);
        if let Some(mut p) = fused {
            let mut tail_len = 0u64;
            if self.broken.is_none() {
                self.note_for_sources(&assembled).await;
            }
            if self.broken.is_none()
                && let Some(tail) = to_app.take()
            {
                tail_len = tail.len() as u64;
                if p.bytes.is_empty() {
                    p.bytes = tail;
                } else {
                    p.bytes.extend_from_slice(&tail);
                }
            }
            if !p.bytes.is_empty() {
                if !self.app.reply(p.bytes).await {
                    self.gone = cut_off(
                        &mut self.app,
                        &mut self.engine,
                        &mut self.broken,
                        self.last_event_at,
                    )
                    .await;
                } else if p.ends {
                    self.last_event_at
                        .get_or_insert(self.app.progress.bytes.saturating_sub(tail_len));
                }
            }
            if !self.gone && self.broken.is_none() && to_app.is_none() {
                // Close the body channel before background accounting so the HTTP layer can flush
                // the terminating chunk immediately.
                self.app.close();
                self.finish_accounting(assembled, None).await;
                return;
            }
        }
        if let Some(tail) = &to_app {
            self.app.progress.bytes += tail.len() as u64;
        }
        let cancelled = self.gone
            && !self
                .last_event_at
                .is_some_and(|at| self.app.taken.reached(at));
        if cancelled {
            self.finish_cancelled(assembled).await;
            return;
        }
        self.finish_accounting(assembled, to_app).await;
    }

    // The reply's end goes out with the bytes before the accounting here, so the request is
    // noted for sources first: a client may report it the moment it has the end.
    async fn note_for_sources(&mut self, assembled: &kaeso_core::assembler::Assembled) {
        let Some(r) = assembled.response.as_ref() else {
            return;
        };
        if !self.engine.has_final_usage(&self.tally) {
            return;
        }
        let duration = self.app.arrived.elapsed();
        let state = self.rec.state().clone();
        let o = self.rec.outcome();
        if o.sources_noted {
            return;
        }
        let Some(mut note) = crate::services::observed::GatewayRequest::of(o) else {
            return;
        };
        o.sources_noted = true;
        note.failed = false;
        note.usage = Some(r.usage.clone());
        note.models.push(r.model.clone());
        note.duration = duration;
        crate::services::observed::note_soon(&state, note)
            .wait()
            .await;
    }

    async fn finish_cancelled(mut self, assembled: kaeso_core::assembler::Assembled) {
        let ended = self.app.arrived.elapsed();
        let (estimate, reported) = self.record_outcome(assembled);
        if estimate {
            let count = reported.total_input_tokens() == 0;
            if count {
                self.rec.outcome().duration = ended;
            }
            let input = if count {
                Some(self.counter.count().await.tokens())
            } else {
                None
            };
            let o = self.rec.outcome();
            o.usage = Some(estimate::complete(&reported, &self.tally, input));
            o.usage_estimated = true;
            count_estimate(EstimateReason::Cancelled, o.provider.unwrap_or_default());
        }
        if self.app.stalled {
            let provider = self.rec.outcome().provider.unwrap_or_default();
            metrics::counter!(crate::metrics::STREAM_FAILURES, "kind" => CLIENT_STALLED, "provider" => provider)
                .increment(1);
            self.rec.finish_stalled().await;
        } else {
            self.rec.finish_cancelled().await;
        }
    }

    fn record_outcome(
        &mut self,
        assembled: kaeso_core::assembler::Assembled,
    ) -> (bool, kaeso_kpp::Usage) {
        let kind = self.rec.outcome().kind;
        // Image, speech and transcript streams report usage on their last event, or bill by unit.
        let media = matches!(
            kind,
            RequestKind::Image | RequestKind::Speech | RequestKind::Transcription
        );
        let estimate = !media
            && !self.tally.has_final_usage()
            && (assembled.complete
                || self.gone
                || (self.broken.is_some() && assembled.response.is_some()));
        if let (Some(r), Some(kept)) = (&assembled.response, self.rec.kept.as_mut()) {
            kept.response_id = Some(r.id.clone());
        }
        let o = self.rec.outcome();
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
                        units.longest_input_ms = None;
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
        if media
            && assembled.complete
            && let StreamEngine::Codec { streamed, .. } = &mut self.engine
        {
            let model = o.response_model.clone().unwrap_or_default();
            let streamed = std::mem::replace(streamed, StreamedMedia::new(false, false));
            o.media_reply_logged = streamed.reply(&model, o.content_type.as_deref());
        }
        o.stream_progress = Some(self.app.progress);
        o.stream_processing = Some(self.processing.0);
        let reported = o.usage.clone().unwrap_or_default();
        (estimate, reported)
    }

    async fn finish_accounting(
        mut self,
        assembled: kaeso_core::assembler::Assembled,
        mut to_app: Option<Vec<u8>>,
    ) {
        let ended = self.app.arrived.elapsed();
        let (estimate, reported) = self.record_outcome(assembled);
        // A stream that ended without a stop still keeps what it had, before the app gets the end.
        if let Some(c) = self.carry.take()
            && self.broken.is_none()
        {
            c.keep().await;
        }
        let count = estimate && reported.total_input_tokens() == 0;
        if count {
            if let Some(tail) = to_app.take()
                && !tail.is_empty()
            {
                self.app.deliver(Bytes::from(tail)).await;
            }
            self.rec.outcome().duration = ended;
        }
        if estimate {
            let input = if count {
                Some(self.counter.count().await.tokens())
            } else {
                None
            };
            let reason = if self.broken.is_some() {
                EstimateReason::Failed
            } else {
                EstimateReason::Unreported
            };
            let o = self.rec.outcome();
            o.usage = Some(estimate::complete(&reported, &self.tally, input));
            o.usage_estimated = true;
            count_estimate(reason, o.provider.unwrap_or_default());
        }
        if let (Some(b), Some(aborted)) = (&self.broken, &self.aborted) {
            match self.first.take() {
                Some(f) => {
                    let _ = f.send(Some(b.err.clone().with_status(b.status)));
                }
                None => aborted.set(),
            }
        }
        if let Some(f) = self.first.take() {
            let _ = f.send(None);
        }
        match self.broken {
            Some(b) => {
                let provider = self.rec.outcome().provider.unwrap_or_default();
                metrics::counter!(crate::metrics::STREAM_FAILURES, "kind" => b.kind.clone(), "provider" => provider)
                    .increment(1);
                self.rec.fail_as(&b.kind, &b.err, b.status).await;
            }
            None => self.rec.finish(200).await,
        }
        if let Some(tail) = to_app
            && !tail.is_empty()
        {
            self.app.deliver(Bytes::from(tail)).await;
        }
    }
}

/// Pushes any already-decoded or already-buffered SSE chunks into `tx` before the response is
/// returned so the HTTP layer writes headers and the first bytes together without a task hop;
/// spawns the pump only when more chunks or async finalization remain.
pub(super) async fn start(parts: Parts, tx: Sender<Bytes>, rec: Recording) {
    let state = rec.state().clone();
    let mut pump = Pump::new(parts, tx, rec);
    let pending = if let Some(init) = pump.passthrough_initial.take() {
        pump.process_batch(BatchStart::PassthroughInitial(init))
            .await
    } else if let Some(init) = pump.initial.take() {
        pump.process_batch(BatchStart::Initial(init)).await
    } else if !pump.app.cut.is_cancelled()
        && let Some(chunks) = pump.chunks.as_mut()
        && let Poll::Ready(first) = poll_next_now(chunks).await
    {
        pump.process_batch(BatchStart::Chunk(first)).await
    } else {
        None
    };
    if pump.done
        && pump.can_finish_before_accounting()
        && !pump.gone
        && pump.engine.has_final_usage(&pump.tally)
    {
        Box::pin(pump.finalize(pending)).await;
    } else {
        state.in_flight.spawn(pump.run(pending));
    }
}

pub(super) async fn pump(parts: Parts, tx: Sender<Bytes>, rec: Recording) {
    Pump::new(parts, tx, rec).run(None).await;
}

async fn cut_off<E: ErrorEncoder + ?Sized>(
    app: &mut ToApp,
    encoder: &mut E,
    broken: &mut Option<Broken>,
    last_event_at: Option<u64>,
) -> bool {
    if app.left() {
        return true;
    }
    if last_event_at.is_none() {
        end_shutdown(app, encoder, broken).await;
    }
    app.is_closed()
}

async fn end_shutdown<E: ErrorEncoder + ?Sized>(
    app: &mut ToApp,
    encoder: &mut E,
    broken: &mut Option<Broken>,
) -> bool {
    app.stopping = true;
    let err = shutting_down();
    broken.get_or_insert_with(|| Broken::shutdown(err.clone()));
    let bytes = encoder.encode_stream_error(&err, broken);
    app.reply(bytes).await
}

async fn end_capped<E: ErrorEncoder + ?Sized>(
    app: &mut ToApp,
    encoder: &mut E,
    broken: &mut Option<Broken>,
) -> bool {
    let err =
        KppError::provider("The reply reached this install's limit for a stream; Kaeso ended it.")
            .with_status(502);
    broken.get_or_insert_with(|| Broken::capped(err.clone()));
    let bytes = encoder.encode_stream_error(&err, broken);
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
            tx: Some(tx),
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

    fn responses_stream(name: &str) -> Vec<u8> {
        let path = kaeso_testkit::mock_provider::kpp_fixtures()
            .join(format!("openai_responses/streams/{name}.sse"));
        std::fs::read(path).unwrap()
    }

    async fn peek_at(bytes: &[u8]) -> (Option<KppError>, Vec<u8>) {
        let mut chunks = kaeso_upstream::ByteStream::replay(
            bytes
                .split_inclusive(|b| *b == b'\n')
                .map(Bytes::copy_from_slice)
                .collect(),
        );
        let ctx = kaeso_codecs::EgressCtx::for_kind(
            RequestKind::Generate,
            kaeso_kpp::StreamMode::on(false),
        );
        let decoder =
            kaeso_codecs::dispatch::stream_decoder(DialectId::OpenAiResponses, &ctx).unwrap();
        let found = peek(&mut chunks, decoder, Duration::from_secs(15)).await;
        let mut replayed = Vec::new();
        while let Some(chunk) = chunks.next().await {
            replayed.extend_from_slice(&chunk.unwrap());
        }
        (found, replayed)
    }

    #[tokio::test]
    async fn an_error_event_that_ends_the_stream_before_any_output_fails_the_attempt() {
        let (found, _) = peek_at(&responses_stream("error-at-start")).await;
        let err = found.expect("the error before any output");
        assert_eq!(err.kind, kaeso_kpp::ErrorKind::ProviderError);
        assert!(
            err.message
                .starts_with("An error occurred while processing"),
            "{}",
            err.message
        );
    }

    #[tokio::test]
    async fn a_stream_with_output_or_without_an_error_is_left_to_the_app() {
        for name in ["error-mid-stream", "text-basic-stream"] {
            let stream = responses_stream(name);
            let (found, replayed) = peek_at(&stream).await;
            assert!(found.is_none(), "{name}: {found:?}");
            assert_eq!(replayed, stream, "{name}");
        }
        // A stream cut off after its start is the pump's to report, as it would be later on.
        let stream = responses_stream("error-at-start");
        let error = b"event: error\n";
        let cut = stream
            .windows(error.len())
            .position(|w| w == error)
            .unwrap();
        let (found, replayed) = peek_at(&stream[..cut]).await;
        assert!(found.is_none(), "{found:?}");
        assert_eq!(replayed, &stream[..cut]);
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

    #[tokio::test]
    async fn stream_body_reports_aborted_when_channel_closes_with_abort_set() {
        use http_body_util::BodyExt;
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let aborted = Aborted::default();
        let mut body = body(rx, Taken::default(), aborted.clone());
        tx.send(Bytes::from_static(b"audio")).await.unwrap();
        aborted.set();
        drop(tx);
        let first = body.frame().await.unwrap().unwrap();
        assert_eq!(first.into_data().unwrap(), Bytes::from_static(b"audio"));
        assert!(body.frame().await.unwrap().is_err());
        assert!(body.frame().await.is_none());
    }

    #[tokio::test]
    async fn peek_keep_and_peek_passthrough_match_peek_behaviour() {
        let ctx = kaeso_codecs::EgressCtx::for_kind(
            RequestKind::Generate,
            kaeso_kpp::StreamMode::on(false),
        );
        let enc_ctx =
            kaeso_codecs::EncodeCtx::new(RequestKind::Generate, kaeso_kpp::StreamMode::on(false));

        let err_stream = responses_stream("error-at-start");
        let lines = |bytes: &[u8]| {
            kaeso_upstream::ByteStream::replay(
                bytes
                    .split_inclusive(|b| *b == b'\n')
                    .map(Bytes::copy_from_slice)
                    .collect(),
            )
        };

        let mut chunks = lines(&err_stream);
        let dec = kaeso_codecs::dispatch::stream_decoder(DialectId::OpenAiResponses, &ctx).unwrap();
        let err = match peek_keep(&mut chunks, dec, Duration::from_secs(15)).await {
            Err(e) => e,
            Ok(_) => panic!("expected error before output"),
        };
        assert_eq!(err.kind, kaeso_kpp::ErrorKind::ProviderError);

        let mut chunks = lines(&err_stream);
        let pass =
            kaeso_codecs::passthrough::for_stream(DialectId::OpenAiResponses, &ctx, &enc_ctx)
                .unwrap();
        let err = match peek_passthrough(&mut chunks, pass, Duration::from_secs(15)).await {
            Err(e) => e,
            Ok(_) => panic!("expected error before output"),
        };
        assert_eq!(err.kind, kaeso_kpp::ErrorKind::ProviderError);

        // Output-bearing stream keeps decoded events / raw bytes without re-reading them.
        let basic = responses_stream("text-basic-stream");
        let mut chunks = lines(&basic);
        let dec = kaeso_codecs::dispatch::stream_decoder(DialectId::OpenAiResponses, &ctx).unwrap();
        let Peeked::Decoded { initial, .. } = peek_keep(&mut chunks, dec, Duration::from_secs(15))
            .await
            .unwrap()
        else {
            panic!("expected decoded initial events");
        };
        assert!(!initial.events.is_empty() && initial.received > 0);

        let mut chunks = lines(&basic);
        let pass =
            kaeso_codecs::passthrough::for_stream(DialectId::OpenAiResponses, &ctx, &enc_ctx)
                .unwrap();
        let (mut pass, initial) = peek_passthrough(&mut chunks, pass, Duration::from_secs(15))
            .await
            .unwrap();
        let mut full = initial.bytes;
        while let Some(chunk) = chunks.next().await {
            let step = pass.feed(&chunk.unwrap(), &mut full);
            assert!(step.error.is_none());
        }
        let step = pass.finish(&mut full);
        assert!(step.error.is_none());
        assert_eq!(full, basic);

        // Truncated stream before output is put back by `peek_keep`.
        let error_marker = b"event: error\n";
        let cut = err_stream
            .windows(error_marker.len())
            .position(|w| w == error_marker)
            .unwrap();
        let mut chunks = lines(&err_stream[..cut]);
        let dec = kaeso_codecs::dispatch::stream_decoder(DialectId::OpenAiResponses, &ctx).unwrap();
        let res = peek_keep(&mut chunks, dec, Duration::from_secs(15))
            .await
            .unwrap();
        assert!(matches!(res, Peeked::PutBack));
    }

    fn future_of_2<A, B, F: Future>(_: impl FnOnce(A, B) -> F) -> usize {
        std::mem::size_of::<F>()
    }

    fn future_of_3<A, B, C, F: Future>(_: impl FnOnce(A, B, C) -> F) -> usize {
        std::mem::size_of::<F>()
    }

    #[test]
    fn an_open_stream_does_not_hold_the_memory_of_the_work_that_ends_it() {
        const OPEN_STREAM: usize = 32 << 10;
        assert!(future_of_2(Pump::run) < OPEN_STREAM);
        assert!(future_of_3(pump) < OPEN_STREAM);
        assert!(future_of_3(start) < OPEN_STREAM);
    }
}
