use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::{Body, Bytes, HttpBody};
use axum::extract::{Request, State};
use axum::http::{Version, header};
use axum::middleware::Next;
use axum::response::Response;
use http_body_util::BodyExt as _;
use hyper::body::{Frame, SizeHint};
use tokio::sync::oneshot;
use tokio::time::{Instant, Sleep};

#[derive(Clone, Copy, Debug)]
pub struct BodyWaits {
    pub between_frames: Duration,
    pub whole_body: Duration,
}

// What a body the handler left unread may still send before the gateway stops reading it.
const DRAIN_LIMIT: u64 = 1 << 30;

pub async fn middleware(State(waits): State<BodyWaits>, req: Request, next: Next) -> Response {
    let deadline = Instant::now() + waits.whole_body;
    // HTTP/2 refuses the rest of a stream on its own (`RST_STREAM` with `NO_ERROR`) and the
    // client keeps the answer, so only HTTP/1 needs the rest read.
    let returns = req.version() < Version::HTTP_2;
    let client_waits = req
        .headers()
        .get(header::EXPECT)
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"100-continue"));
    let mut unread = None;
    let response = next
        .run(req.map(|body| {
            if body.is_end_stream() {
                return body;
            }
            let paced = Paced {
                body,
                between_frames: waits.between_frames,
                deadline,
                waiting_since: None,
                alarm: None,
            };
            if !returns {
                return Body::new(paced);
            }
            let (give_back, rx) = oneshot::channel();
            unread = Some(rx);
            Body::new(GivesBackUnread {
                body: Some(paced),
                give_back: Some(give_back),
                polled: false,
                ended: false,
            })
        }))
        .await;
    // A proxy that streams the body, such as Traefik, writes all of it before it reads the
    // answer; closing the connection under that write makes the proxy answer 502 itself. A
    // client waiting for `100 Continue` hasn't sent the body, so it isn't asked for it.
    if let Some(Ok(Unread { body, polled })) = unread.map(|mut rx| rx.try_recv())
        && (polled || !client_waits)
        && body
            .size_hint()
            .exact()
            .is_none_or(|length| length <= DRAIN_LIMIT)
    {
        drain(Body::new(body)).await;
    }
    response
}

async fn drain(mut body: Body) {
    let mut drained: u64 = 0;
    while drained <= DRAIN_LIMIT {
        match body.frame().await {
            Some(Ok(frame)) => {
                if let Some(data) = frame.data_ref() {
                    drained = drained.saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
                }
            }
            Some(Err(_)) | None => return,
        }
    }
}

struct Unread {
    body: Paced,
    polled: bool,
}

struct GivesBackUnread {
    body: Option<Paced>,
    give_back: Option<oneshot::Sender<Unread>>,
    polled: bool,
    ended: bool,
}

impl HttpBody for GivesBackUnread {
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

impl Drop for GivesBackUnread {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::http::StatusCode;
    use axum::routing::post;
    use futures::StreamExt as _;
    use tower::ServiceExt as _;

    use super::*;

    const WAITS: BodyWaits = BodyWaits {
        between_frames: Duration::from_secs(30),
        whole_body: Duration::from_secs(60),
    };

    // `frames` frames of `size` bytes, then the end, or a body that never ends; counts the frames
    // read from it.
    fn counted(frames: usize, size: usize, polled: Arc<AtomicUsize>, ends: bool) -> Body {
        let data = Bytes::from(vec![b'a'; size]);
        let sent = futures::stream::iter(0..frames).map(move |_| {
            polled.fetch_add(1, Ordering::Relaxed);
            Ok::<_, std::io::Error>(Frame::data(data.clone()))
        });
        if ends {
            Body::new(http_body_util::StreamBody::new(sent))
        } else {
            Body::new(http_body_util::StreamBody::new(
                sent.chain(futures::stream::pending()),
            ))
        }
    }

    struct Declared(u64, Arc<AtomicUsize>);

    impl HttpBody for Declared {
        type Data = Bytes;
        type Error = std::io::Error;

        fn poll_frame(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
            self.1.fetch_add(1, Ordering::Relaxed);
            Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"abcd")))))
        }

        fn size_hint(&self) -> SizeHint {
            SizeHint::with_exact(self.0)
        }
    }

    fn app(waits: BodyWaits) -> Router {
        Router::new()
            .route("/refuse", post(|| async { StatusCode::UNAUTHORIZED }))
            .route(
                "/limit",
                post(|body: Body| async move {
                    let e = crate::http::body::read(body, 5).await.unwrap_err();
                    assert!(crate::http::body::over_limit(&e));
                    StatusCode::PAYLOAD_TOO_LARGE
                }),
            )
            .route(
                "/one",
                post(|mut body: Body| async move {
                    body.frame().await;
                    StatusCode::PAYLOAD_TOO_LARGE
                }),
            )
            .route(
                "/keep",
                post(|body: Body| async move {
                    tokio::spawn(async move {
                        let _held = body;
                        futures::future::pending::<()>().await;
                    });
                    StatusCode::ACCEPTED
                }),
            )
            .layer(axum::middleware::from_fn_with_state(waits, middleware))
    }

    async fn send(
        path: &str,
        body: Body,
        expect: bool,
        version: Version,
        waits: BodyWaits,
    ) -> StatusCode {
        let mut req = Request::post(path).version(version);
        if expect {
            req = req.header(header::EXPECT, "100-continue");
        }
        app(waits)
            .oneshot(req.body(body).unwrap())
            .await
            .unwrap()
            .status()
    }

    async fn frames_read(path: &str, frames: usize, expect: bool, version: Version) -> usize {
        let polled = Arc::new(AtomicUsize::new(0));
        let body = counted(frames, 4, polled.clone(), true);
        send(path, body, expect, version, WAITS).await;
        polled.load(Ordering::Relaxed)
    }

    #[tokio::test]
    async fn a_refused_body_is_read_to_its_end_before_the_answer() {
        assert_eq!(
            frames_read("/refuse", 10, false, Version::HTTP_11).await,
            10
        );
        assert_eq!(frames_read("/limit", 10, false, Version::HTTP_11).await, 10);
        assert_eq!(frames_read("/one", 10, false, Version::HTTP_11).await, 10);
    }

    #[tokio::test]
    async fn a_client_waiting_for_100_continue_is_asked_for_its_body_only_once_it_was_read() {
        assert_eq!(frames_read("/refuse", 10, true, Version::HTTP_11).await, 0);
        assert_eq!(frames_read("/one", 10, true, Version::HTTP_11).await, 10);
    }

    #[tokio::test]
    async fn http2_answers_without_reading_the_rest() {
        assert_eq!(frames_read("/refuse", 10, false, Version::HTTP_2).await, 0);
    }

    #[tokio::test]
    async fn a_body_that_declares_more_than_can_be_read_is_not_read() {
        let polled = Arc::new(AtomicUsize::new(0));
        let body = Body::new(Declared(DRAIN_LIMIT + 1, polled.clone()));
        let status = send("/refuse", body, false, Version::HTTP_11, WAITS).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(polled.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn reading_the_rest_stops_after_a_gibibyte_however_the_handler_read() {
        let frame = 64 << 20;
        let gibibyte_and_one = usize::try_from(DRAIN_LIMIT).unwrap() / frame + 1;
        for (path, read_by_handler) in [("/refuse", 0), ("/limit", 1)] {
            let polled = Arc::new(AtomicUsize::new(0));
            let body = counted(usize::MAX, frame, polled.clone(), true);
            send(path, body, false, Version::HTTP_11, WAITS).await;
            assert_eq!(
                polled.load(Ordering::Relaxed),
                read_by_handler + gibibyte_and_one,
                "{path}"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_answer_goes_out_at_the_deadline_when_the_rest_never_comes() {
        let polled = Arc::new(AtomicUsize::new(0));
        let body = counted(2, 4, polled.clone(), false);
        let waits = BodyWaits {
            between_frames: Duration::from_secs(5),
            whole_body: Duration::from_secs(60),
        };
        let status = send("/refuse", body, false, Version::HTTP_11, waits).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(polled.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn a_body_the_handler_keeps_is_left_to_it() {
        let polled = Arc::new(AtomicUsize::new(0));
        let body = counted(10, 4, polled.clone(), false);
        let status = tokio::time::timeout(
            Duration::from_secs(5),
            send("/keep", body, false, Version::HTTP_11, WAITS),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(polled.load(Ordering::Relaxed), 0);
    }
}
