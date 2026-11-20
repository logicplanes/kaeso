use axum::body::Body;
use axum::body::HttpBody as _;
use bytes::{Bytes, BytesMut};
use http_body_util::{BodyExt as _, LengthLimitError, Limited};

// What an oversized body may still send before the gateway stops reading it. Beyond this, and
// past the body's read deadline, the connection closes.
const DRAIN_LIMIT: u64 = 1 << 30;

// Collecting the frames and joining them at the end holds a large upload twice; copying each
// frame into a buffer sized from the body's length as it arrives holds it once.
pub async fn read(body: Body, limit: usize) -> Result<Bytes, axum::Error> {
    read_releasing(body, limit, || {}).await
}

// `release` runs before the rest of a refused body is read, so what the caller holds for the
// body isn't kept while it drains.
pub async fn read_releasing(
    mut body: Body,
    limit: usize,
    release: impl FnOnce(),
) -> Result<Bytes, axum::Error> {
    let limit_bytes = u64::try_from(limit).unwrap_or(u64::MAX);
    let declared = body.size_hint().exact();
    let read = match declared {
        Some(length) if length > limit_bytes => Err(axum::Error::new(DeclaredTooLarge)),
        _ => collect(&mut body, limit).await,
    };
    if read.as_ref().is_err_and(|e| over_limit(e)) {
        release();
        if declared.is_none_or(|length| length <= limit_bytes.saturating_add(DRAIN_LIMIT)) {
            drain(&mut body).await;
        }
    }
    read
}

async fn collect(body: &mut Body, limit: usize) -> Result<Bytes, axum::Error> {
    let hint = body.size_hint();
    let expected = hint.exact().or(hint.upper()).unwrap_or(hint.lower());
    let mut body = Limited::new(body, limit);
    let mut first: Option<Bytes> = None;
    let mut joined: Option<BytesMut> = None;
    while let Some(frame) = body.frame().await {
        let Ok(data) = frame.map_err(axum::Error::new)?.into_data() else {
            continue;
        };
        if data.is_empty() {
            continue;
        }
        if let Some(buf) = joined.as_mut() {
            buf.extend_from_slice(&data);
            continue;
        }
        match first.take() {
            None => first = Some(data),
            Some(earlier) => {
                let capacity = usize::try_from(expected)
                    .unwrap_or(limit)
                    .min(limit)
                    .max(earlier.len() + data.len());
                let mut buf = BytesMut::with_capacity(capacity);
                buf.extend_from_slice(&earlier);
                buf.extend_from_slice(&data);
                joined = Some(buf);
            }
        }
    }
    Ok(match joined {
        Some(buf) => buf.freeze(),
        None => first.unwrap_or_default(),
    })
}

// A proxy that streams the body to the gateway, such as Traefik, answers the app with its own
// 502 when the gateway closes the connection while the body is still arriving, so the 413 never
// reaches the app. Reading the rest first lets the proxy finish and pass the 413 on.
async fn drain(body: &mut Body) {
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

#[derive(Debug, thiserror::Error)]
#[error("the request body declares a length over the limit")]
struct DeclaredTooLarge;

pub(crate) fn over_limit(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut cause = Some(error);
    while let Some(e) = cause {
        if e.is::<LengthLimitError>() || e.is::<DeclaredTooLarge>() {
            return true;
        }
        cause = e.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunked(chunks: &[&'static [u8]]) -> Body {
        let frames = chunks
            .iter()
            .map(|c| Ok::<_, std::io::Error>(hyper::body::Frame::data(Bytes::from_static(c))))
            .collect::<Vec<_>>();
        Body::new(http_body_util::StreamBody::new(futures::stream::iter(
            frames,
        )))
    }

    #[tokio::test]
    async fn frames_are_joined_in_order_and_one_frame_is_kept_as_it_is() {
        let joined = read(chunked(&[b"ab", b"", b"cd", b"e"]), 10).await.unwrap();
        assert_eq!(joined, Bytes::from_static(b"abcde"));
        let one = Bytes::from_static(b"whole");
        let read_one = read(Body::from(one.clone()), 10).await.unwrap();
        assert_eq!(read_one.as_ptr(), one.as_ptr());
        assert!(read(Body::empty(), 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_body_over_the_limit_is_refused_as_a_length_limit_error() {
        let e = read(chunked(&[b"abc", b"def"]), 5).await.unwrap_err();
        let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(&e);
        let mut found = false;
        while let Some(c) = cause {
            found |= c.is::<http_body_util::LengthLimitError>();
            cause = c.source();
        }
        assert!(found, "{e}");
    }

    fn counted(
        frames: usize,
        size: usize,
        polled: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> Body {
        use futures::StreamExt as _;
        let data = Bytes::from(vec![b'a'; size]);
        let stream = futures::stream::iter(0..frames).map(move |_| {
            polled.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok::<_, std::io::Error>(hyper::body::Frame::data(data.clone()))
        });
        Body::new(http_body_util::StreamBody::new(stream))
    }

    #[tokio::test]
    async fn an_oversized_body_is_read_to_its_end_before_the_refusal() {
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let e = read(counted(10, 4, polled.clone()), 5).await.unwrap_err();
        assert!(over_limit(&e), "{e}");
        assert_eq!(polled.load(std::sync::atomic::Ordering::Relaxed), 10);
    }

    #[tokio::test]
    async fn draining_stops_after_a_gibibyte() {
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let frame = 64 << 20;
        let e = read(counted(usize::MAX, frame, polled.clone()), 5)
            .await
            .unwrap_err();
        assert!(over_limit(&e), "{e}");
        let read = polled.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(read, 1 + usize::try_from(DRAIN_LIMIT).unwrap() / frame + 1);
    }

    // A body that says how long it is, and counts the frames read from it.
    struct Declared {
        length: u64,
        polled: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl axum::body::HttpBody for Declared {
        type Data = Bytes;
        type Error = std::io::Error;

        fn poll_frame(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<hyper::body::Frame<Bytes>, Self::Error>>> {
            let n = self
                .polled
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            std::task::Poll::Ready(
                (n < 4).then(|| Ok(hyper::body::Frame::data(Bytes::from_static(b"abcd")))),
            )
        }

        fn size_hint(&self) -> hyper::body::SizeHint {
            hyper::body::SizeHint::with_exact(self.length)
        }
    }

    #[tokio::test]
    async fn a_body_that_declares_too_much_is_refused_and_read_to_its_end_without_being_kept() {
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let released = std::sync::atomic::AtomicBool::new(false);
        let body = Body::new(Declared {
            length: 16,
            polled: polled.clone(),
        });
        let e = read_releasing(body, 5, || {
            released.store(true, std::sync::atomic::Ordering::Relaxed);
        })
        .await
        .unwrap_err();
        assert!(over_limit(&e), "{e}");
        assert!(released.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(
            polled.load(std::sync::atomic::Ordering::Relaxed),
            5,
            "four frames and the end"
        );
    }

    #[tokio::test]
    async fn a_body_that_declares_more_than_can_be_drained_is_not_read() {
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let body = Body::new(Declared {
            length: DRAIN_LIMIT + 6,
            polled: polled.clone(),
        });
        let e = read(body, 5).await.unwrap_err();
        assert!(over_limit(&e), "{e}");
        assert_eq!(polled.load(std::sync::atomic::Ordering::Relaxed), 0);
    }
}
