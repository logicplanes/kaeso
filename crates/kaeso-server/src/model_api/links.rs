use base64::Engine as _;
use bytes::Bytes;
use kaeso_core::provider::fetchable;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{AudioFormat, ErrorKind, ImageDelivery, KppError, Reply, Request, Source};
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};
use url::Url;

use super::errors::error;
use crate::state::AppState;

/// The provider's result links Kaeso fetches before answering: the images an app asked to get
/// as bytes from a provider that answers with links only, and a whole speech reply's audio.
#[derive(Clone, Debug)]
pub(super) struct Fetch {
    hosts: &'static [&'static str],
    provider: &'static str,
    audio: Option<AudioFormat>,
}

impl Fetch {
    /// What to fetch for `req` at `provider`, taking the asked bytes off the request the
    /// provider gets, since it answers with links only.
    pub(super) fn of(req: &mut Request, provider: &ProviderRef, model: &str) -> Option<Fetch> {
        let name = provider.built_in()?.name;
        match req {
            Request::Image(r) if r.delivery == Some(ImageDelivery::Base64) => {
                let hosts = provider.images(model)?.fetch_hosts;
                if hosts.is_empty() {
                    return None;
                }
                r.delivery = None;
                Some(Fetch {
                    hosts,
                    provider: name,
                    audio: None,
                })
            }
            Request::Speech(r) => {
                let hosts = provider.speech(model)?.fetch_hosts;
                (!hosts.is_empty()).then(|| Fetch {
                    hosts,
                    provider: name,
                    audio: r.format.clone(),
                })
            }
            _ => None,
        }
    }

    /// Replaces the reply's links with the files they point at.
    pub(super) async fn resolve(
        &self,
        state: &AppState,
        audio_format: Option<&AudioFormat>,
        reply: &mut Reply,
    ) -> Result<(), KppError> {
        self.resolve_with(audio_format, reply, |link| self.get(state, link))
            .await
    }

    async fn resolve_with<F, Fut>(
        &self,
        audio_format: Option<&AudioFormat>,
        reply: &mut Reply,
        get: F,
    ) -> Result<(), KppError>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<(Option<String>, Bytes), KppError>>,
    {
        match reply {
            Reply::Image(r) => {
                let fetched = futures::future::try_join_all(r.images.iter().map(|i| {
                    let link = match &i.source {
                        Source::Url(link) => Some(link.clone()),
                        _ => None,
                    };
                    let fut = link.map(&get);
                    async move {
                        match fut {
                            Some(f) => f.await.map(Some),
                            None => Ok(None),
                        }
                    }
                }))
                .await?;
                for (image, got) in r.images.iter_mut().zip(fetched) {
                    if let Some((media_type, bytes)) = got {
                        let media_type = media_type
                            .filter(|t| t.starts_with("image/"))
                            .unwrap_or_else(|| "image/png".to_owned());
                        image.source = Source::base64(media_type, encode(&bytes));
                    }
                }
            }
            Reply::Speech(r) => {
                if let Source::Url(link) = &r.audio {
                    let (media_type, bytes) = get(link.clone()).await?;
                    let asked = self
                        .audio
                        .as_ref()
                        .or(audio_format)
                        .and_then(AudioFormat::media_type);
                    let media_type = asked
                        .map(str::to_owned)
                        .or(media_type.filter(|t| t.starts_with("audio/")))
                        .unwrap_or_else(|| "application/octet-stream".to_owned());
                    r.audio = Source::base64(media_type, encode(&bytes));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn refused(&self) -> KppError {
        tracing::warn!(
            provider = self.provider,
            "a result link points outside the provider's documented hosts"
        );
        error(
            ErrorKind::ProviderError,
            502,
            format!(
                "{} answered with a link Kaeso does not fetch.",
                self.provider
            ),
        )
    }

    // Only https, only the hosts the provider documents for its results, and no credentials or
    // other ports in the link.
    fn target(&self, link: &str) -> Result<(Url, String), KppError> {
        let mut url = Url::parse(link).map_err(|_| self.refused())?;
        let host = url
            .host_str()
            .map(str::to_owned)
            .ok_or_else(|| self.refused())?;
        if !fetchable(self.hosts, &host)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some_and(|p| p != 443)
        {
            return Err(self.refused());
        }
        // QwenCloud's references show some result links as http on the same buckets, which
        // serve the same object over https.
        match url.scheme() {
            "https" => {}
            "http" => url.set_scheme("https").map_err(|()| self.refused())?,
            _ => return Err(self.refused()),
        }
        let path = match url.query() {
            Some(q) => format!("{}?{q}", url.path()),
            None => url.path().to_owned(),
        };
        let mut origin = url;
        origin.set_path("/");
        origin.set_query(None);
        origin.set_fragment(None);
        Ok((origin, path))
    }

    // Through the guard that keeps calls off private networks, without the provider's key,
    // following no redirect, and bounded by the provider response limit and the request timeout.
    async fn get(
        &self,
        state: &AppState,
        link: String,
    ) -> Result<(Option<String>, Bytes), KppError> {
        let (origin, path) = self.target(&link)?;
        let u = &state.config.upstream;
        let empty = secrecy::SecretString::from(String::new());
        let resp = state
            .upstream
            .send(Call {
                base_url: &origin,
                auth: None,
                guarded: true,
                credential: &empty,
                request: Outgoing {
                    method: "GET",
                    path,
                    headers: kaeso_codecs::HeaderList::new(),
                    body: Bytes::new(),
                },
                stream: false,
                timeouts: Timeouts {
                    request: u.request_timeout,
                    first_byte: u.first_byte_timeout,
                    idle: u.stream_idle_timeout,
                },
                max_body: state.config.body_limits.provider_response.as_usize(),
            })
            .await
            .map_err(|e| super::pipeline::upstream_error(&e))?;
        if !(200..300).contains(&resp.status) {
            tracing::warn!(
                provider = self.provider,
                status = resp.status,
                "a result link did not download"
            );
            return Err(error(
                ErrorKind::ProviderError,
                502,
                format!(
                    "{} did not hand over the file it made; try again shortly.",
                    self.provider
                ),
            ));
        }
        let body = match resp.body {
            ResponseBody::Full(b) => b,
            ResponseBody::Stream(_) => Bytes::new(),
        };
        let media_type = resp
            .headers
            .get("content-type")
            .map(|t| t.split(';').next().unwrap_or(t).trim().to_ascii_lowercase());
        Ok((media_type, body))
    }
}

fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use kaeso_kpp::{GeneratedImage, ImageOperation, ImageRequest, ImageResponse, SpeechResponse};

    use super::*;

    fn fetch(audio: Option<AudioFormat>) -> Fetch {
        Fetch {
            hosts: &["dashscope-*.aliyuncs.com"],
            provider: "QwenCloud",
            audio,
        }
    }

    #[test]
    fn only_https_links_on_the_documented_hosts_are_fetched() {
        let f = fetch(None);
        let (origin, path) = f
            .target("http://dashscope-result-bj.oss-cn-beijing.aliyuncs.com/a/b.wav?Expires=1&Signature=s")
            .unwrap();
        assert_eq!(
            origin.as_str(),
            "https://dashscope-result-bj.oss-cn-beijing.aliyuncs.com/"
        );
        assert_eq!(path, "/a/b.wav?Expires=1&Signature=s");
        for link in [
            "https://example.com/a.png",
            "https://dashscope-x.aliyuncs.com.example.com/a.png",
            "https://user@dashscope-x.aliyuncs.com/a.png",
            "https://dashscope-x.aliyuncs.com:8443/a.png",
            "ftp://dashscope-x.aliyuncs.com/a.png",
            "https://127.0.0.1/a.png",
            "not a link",
        ] {
            let err = f.target(link).unwrap_err();
            assert_eq!(err.status, Some(502), "{link}");
            assert_eq!(
                err.message,
                "QwenCloud answered with a link Kaeso does not fetch."
            );
        }
    }

    #[tokio::test]
    async fn links_become_the_files_they_point_at() {
        let f = fetch(None);
        let mut reply = Reply::Image(ImageResponse::new(
            "m",
            vec![
                GeneratedImage::new(Source::Url("https://a".into())),
                GeneratedImage::new(Source::base64("image/webp", "AAAA")),
                GeneratedImage::new(Source::Url("https://b".into())),
            ],
        ));
        f.resolve_with(None, &mut reply, |link| async move {
            let t = (link == "https://a").then(|| "image/jpeg".to_owned());
            Ok((t, Bytes::from_static(b"img")))
        })
        .await
        .unwrap();
        let Reply::Image(r) = &reply else { panic!() };
        let sources: Vec<_> = r.images.iter().map(|i| i.source.clone()).collect();
        assert_eq!(
            sources,
            vec![
                Source::base64("image/jpeg", "aW1n"),
                Source::base64("image/webp", "AAAA"),
                Source::base64("image/png", "aW1n"),
            ]
        );

        let mut speech = Reply::Speech(SpeechResponse::new("m", Source::Url("https://c".into())));
        f.resolve_with(Some(&AudioFormat::Wav), &mut speech, |_| async {
            Ok((
                Some("application/octet-stream".to_owned()),
                Bytes::from_static(b"RIFF"),
            ))
        })
        .await
        .unwrap();
        let Reply::Speech(s) = &speech else { panic!() };
        assert_eq!(s.audio, Source::base64("audio/wav", "UklGRg=="));

        let failed = f
            .resolve_with(None, &mut reply_with_link(), |_| async {
                Err(error(ErrorKind::ProviderError, 502, "gone"))
            })
            .await
            .unwrap_err();
        assert_eq!(failed.message, "gone");
    }

    fn reply_with_link() -> Reply {
        Reply::Image(ImageResponse::new(
            "m",
            vec![GeneratedImage::new(Source::Url("https://a".into()))],
        ))
    }

    #[test]
    fn only_asked_bytes_and_whole_speech_are_fetched() {
        let qwen = ProviderRef::from(kaeso_core::provider::provider("alibaba").unwrap());
        let openai = ProviderRef::from(kaeso_core::provider::provider("openai").unwrap());
        let mut asked =
            ImageRequest::new("wan2.7-image", ImageOperation::Generate).with_prompt("p");
        asked.delivery = Some(ImageDelivery::Base64);
        let mut req = Request::Image(asked.clone());
        assert!(Fetch::of(&mut req, &qwen, "wan2.7-image").is_some());
        let Request::Image(sent) = &req else { panic!() };
        assert_eq!(sent.delivery, None);
        let mut req = Request::Image(asked.clone());
        assert!(Fetch::of(&mut req, &openai, "gpt-image-2").is_none());
        let mut links = asked;
        links.delivery = None;
        assert!(Fetch::of(&mut Request::Image(links), &qwen, "wan2.7-image").is_none());
        let speech =
            kaeso_kpp::SpeechRequest::new("qwen3-tts-flash", kaeso_kpp::SpeechOperation::Speech);
        assert!(
            Fetch::of(
                &mut Request::Speech(speech.clone()),
                &qwen,
                "qwen3-tts-flash"
            )
            .is_some()
        );
        assert!(Fetch::of(&mut Request::Speech(speech), &openai, "tts-1").is_none());
    }
}
