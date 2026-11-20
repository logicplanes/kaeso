use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::{Body, Bytes, HttpBody};
use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use hyper::body::{Frame, SizeHint};
use tokio::sync::oneshot;
use tokio::time::{Instant, Sleep};

#[derive(Clone, Copy, Debug)]
pub struct BodyWaits {
    pub between_frames: Duration,
    pub whole_body: Duration,
}

pub async fn middleware(State(waits): State<BodyWaits>, req: Request, next: Next) -> Response {
    let deadline = Instant::now() + waits.whole_body;
    let client_waits = req
        .headers()
        .get(header::EXPECT)
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"100-continue"));
    let (give_back, mut unread) = oneshot::channel();
    let response = next
        .run(req.map(|body| {
            if body.is_end_stream() {
                return body;
            }
            Body::new(Returned {
                body: Some(Paced {
                    body,
                    between_frames: waits.between_frames,
                    deadline,
                    waiting_since: None,
                    alarm: None,
                }),
                give_back: Some(give_back),
                polled: false,
                ended: false,
            })
        }))
        .await;
    // An answer sent before the body arrived breaks a proxy still sending it, which then answers
    // 502 itself. A client waiting for `100 Continue` hasn't sent the body, so it isn't asked.
    if let Ok(Unread { body, polled }) = unread.try_recv()
        && (polled || !client_waits)
        && body
            .size_hint()
            .exact()
            .is_none_or(|length| length <= super::body::DRAIN_LIMIT)
    {
        super::body::drain(&mut Body::new(body)).await;
    }
    response
}

struct Unread {
    body: Paced,
    polled: bool,
}

// Hands a body the handler dropped before its end back to the middleware.
struct Returned {
    body: Option<Paced>,
    give_back: Option<oneshot::Sender<Unread>>,
    polled: bool,
    ended: bool,
}

impl HttpBody for Returned {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, axum::Error>>> {
        self.polled = true;
        let Some(body) = self.body.as_mut() else {
            return Poll::Ready(None);
        };
        let frame = Pin::new(body).poll_frame(cx);
        if matches!(frame, Poll::Ready(None | Some(Err(_)))) {
            self.ended = true;
        }
        frame
    }

    fn is_end_stream(&self) -> bool {
        self.body.as_ref().is_none_or(HttpBody::is_end_stream)
    }

    fn size_hint(&self) -> SizeHint {
        self.body
            .as_ref()
            .map_or_else(|| SizeHint::with_exact(0), HttpBody::size_hint)
    }
}

impl Drop for Returned {
    fn drop(&mut self) {
        if let (Some(body), Some(give_back)) = (self.body.take(), self.give_back.take())
            && !self.ended
            && !body.is_end_stream()
        {
            let _ = give_back.send(Unread {
                body,
                polled: self.polled,
            });
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("the request body did not arrive before its deadline")]
struct PastDeadline;

#[derive(Debug, thiserror::Error)]
#[error("the request body paused for too long between two frames")]
struct Paused;

// One timer for both waits, armed only when the body has to wait and moved on lazily: tokio's
// timer wheel sits behind one lock, and a timer per frame serialises the gateway under load.
struct Paced {
    body: Body,
    between_frames: Duration,
    deadline: Instant,
    waiting_since: Option<Instant>,
    alarm: Option<Pin<Box<Sleep>>>,
}

impl Paced {
    fn due(&self, waiting_since: Instant) -> Instant {
        (waiting_since + self.between_frames).min(self.deadline)
    }

    fn overdue(&self, now: Instant) -> Option<axum::Error> {
        if now >= self.deadline {
            return Some(axum::Error::new(PastDeadline));
        }
        let since = self.waiting_since?;
        (now >= self.due(since)).then(|| axum::Error::new(Paused))
    }

    fn alarm_rang(&mut self, cx: &mut Context<'_>, due: Instant) -> bool {
        let alarm = self
            .alarm
            .get_or_insert_with(|| Box::pin(tokio::time::sleep_until(due)));
        alarm.as_mut().poll(cx).is_ready()
    }
}

impl HttpBody for Paced {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, axum::Error>>> {
        let now = Instant::now();
        if let Some(e) = self.overdue(now) {
            return Poll::Ready(Some(Err(e)));
        }
        if let Poll::Ready(frame) = Pin::new(&mut self.body).poll_frame(cx) {
            self.waiting_since = None;
            return Poll::Ready(frame);
        }
        let since = *self.waiting_since.get_or_insert(now);
        let due = self.due(since);
        // A timer armed for an earlier wait rings early and only then moves on to this wait's end.
        while self.alarm_rang(cx, due) {
            if let Some(e) = self.overdue(Instant::now()) {
                return Poll::Ready(Some(Err(e)));
            }
            if let Some(alarm) = self.alarm.as_mut() {
                alarm.as_mut().reset(due);
            }
        }
        Poll::Pending
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

pub fn timed_out(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut cause = Some(error);
    while let Some(e) = cause {
        if e.is::<Paused>() || e.is::<PastDeadline>() {
            return true;
        }
        cause = e.source();
    }
    false
}
