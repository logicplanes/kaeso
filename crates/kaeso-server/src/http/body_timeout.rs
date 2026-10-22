use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::{Body, Bytes, HttpBody};
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use hyper::body::{Frame, SizeHint};
use tokio::time::{Instant, Sleep};
use tower_http::timeout::{TimeoutBody, TimeoutError};

#[derive(Clone, Copy, Debug)]
pub struct BodyWaits {
    pub between_frames: Duration,
    pub whole_body: Duration,
}

pub async fn middleware(State(waits): State<BodyWaits>, req: Request, next: Next) -> Response {
    let deadline = Instant::now() + waits.whole_body;
    next.run(req.map(|body| {
        let paced = Body::new(TimeoutBody::new(waits.between_frames, body));
        if paced.is_end_stream() {
            return paced;
        }
        Body::new(Deadline {
            body: paced,
            deadline: Box::pin(tokio::time::sleep_until(deadline)),
        })
    }))
    .await
}

#[derive(Debug, thiserror::Error)]
#[error("the request body did not arrive before its deadline")]
struct PastDeadline;

struct Deadline {
    body: Body,
    deadline: Pin<Box<Sleep>>,
}

impl HttpBody for Deadline {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, axum::Error>>> {
        if self.deadline.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Some(Err(axum::Error::new(PastDeadline))));
        }
        Pin::new(&mut self.body).poll_frame(cx)
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
        if e.is::<TimeoutError>() || e.is::<PastDeadline>() {
            return true;
        }
        cause = e.source();
    }
    false
}
