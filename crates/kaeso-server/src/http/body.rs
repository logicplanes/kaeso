use axum::body::Body;
use axum::body::HttpBody as _;
use bytes::{Bytes, BytesMut};
use http_body_util::{BodyExt as _, Limited};

// Collecting the frames and joining them at the end holds a large upload twice; copying each
// frame into a buffer sized from the body's length as it arrives holds it once.
pub async fn read(body: Body, limit: usize) -> Result<Bytes, axum::Error> {
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
}
