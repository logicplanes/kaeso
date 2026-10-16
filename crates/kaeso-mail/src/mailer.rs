use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use secrecy::{ExposeSecret, SecretString};
use tokio::sync::{Semaphore, mpsc, oneshot};

use crate::assets;
use crate::layout::Instance;
use crate::templates::{Email, Rendered};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmtpSecurity {
    Starttls,
    Tls,
    None,
}

#[derive(Clone, Debug)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
    pub credentials: Option<(String, SecretString)>,
}

#[derive(Debug, Default)]
struct Mailbag {
    delivered: Vec<Rendered>,
    failing: HashMap<String, usize>,
}

#[derive(Clone, Debug, Default)]
pub struct MemorySink(Arc<Mutex<Mailbag>>);

impl MemorySink {
    pub fn new() -> MemorySink {
        MemorySink::default()
    }

    pub fn all(&self) -> Vec<Rendered> {
        self.0
            .lock()
            .map(|bag| bag.delivered.clone())
            .unwrap_or_default()
    }

    pub fn fail_next(&self, to: &str, attempts: usize) {
        if let Ok(mut bag) = self.0.lock() {
            *bag.failing.entry(to.to_ascii_lowercase()).or_default() += attempts;
        }
    }

    pub fn to(&self, to: &str) -> Vec<Rendered> {
        self.all()
            .into_iter()
            .filter(|e| e.to.eq_ignore_ascii_case(to))
            .collect()
    }

    fn push(&self, email: Rendered) -> Result<(), MailError> {
        let Ok(mut bag) = self.0.lock() else {
            return Ok(());
        };
        if let Some(left) = bag
            .failing
            .get_mut(&email.to.to_ascii_lowercase())
            .filter(|left| **left > 0)
        {
            *left -= 1;
            return Err(MailError::Smtp(
                "the memory sink refused this attempt".into(),
            ));
        }
        bag.delivered.push(email);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Transport {
    Smtp(SmtpSettings),
    Log,
    Memory(MemorySink),
}

#[derive(Clone)]
enum Sender {
    Smtp(AsyncSmtpTransport<Tokio1Executor>),
    Log,
    Memory(MemorySink),
}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("the email is invalid: {0}")]
    Invalid(String),
    #[error("the SMTP server did not accept the email: {0}")]
    Smtp(String),
    #[error("the SMTP server refused the email for good: {0}")]
    Refused(String),
    #[error("the SMTP settings are invalid: {0}")]
    Config(String),
}

enum Job {
    Deliver(Rendered),
    Flush(oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct Mailer {
    queue: mpsc::Sender<Job>,
    sender: Sender,
    from: Mailbox,
    instance: Instance,
}

impl std::fmt::Debug for Mailer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mailer")
            .field("from", &self.from.to_string())
            .field("instance", &self.instance.name)
            .finish_non_exhaustive()
    }
}

const QUEUE: usize = 1024;
const DELIVERIES: u32 = 16;
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(5),
    Duration::from_secs(30),
];

impl Mailer {
    pub fn start(
        transport: Transport,
        from: &str,
        instance: Instance,
    ) -> Result<Mailer, MailError> {
        let from: Mailbox = from
            .parse()
            .map_err(|e| MailError::Config(format!("KAESO_SMTP_FROM is not an address: {e}")))?;
        let sender = match transport {
            Transport::Smtp(s) => Sender::Smtp(smtp(&s)?),
            Transport::Log => Sender::Log,
            Transport::Memory(m) => Sender::Memory(m),
        };
        let (queue, rx) = mpsc::channel::<Job>(QUEUE);
        let mailer = Mailer {
            queue,
            sender: sender.clone(),
            from: from.clone(),
            instance,
        };
        tokio::spawn(work(rx, sender, from));
        Ok(mailer)
    }

    pub fn send(&self, email: Email) {
        let kind = email.kind.as_str();
        if self
            .queue
            .try_send(Job::Deliver(email.render(&self.instance)))
            .is_err()
        {
            metrics::counter!("kaeso_emails_total", "kind" => kind, "outcome" => "dropped")
                .increment(1);
            tracing::error!(kind, "email queue full; email dropped");
        }
    }

    pub async fn flush(&self) {
        let (done, delivered) = oneshot::channel();
        if self.queue.send(Job::Flush(done)).await.is_ok() {
            let _ = delivered.await;
        }
    }

    pub async fn send_now(&self, email: Email) -> Result<(), MailError> {
        deliver(&self.sender, &self.from, &email.render(&self.instance)).await
    }
}

async fn work(mut rx: mpsc::Receiver<Job>, sender: Sender, from: Mailbox) {
    let slots = Arc::new(Semaphore::new(DELIVERIES as usize));
    while let Some(job) = rx.recv().await {
        match job {
            Job::Deliver(email) => {
                let Ok(slot) = Arc::clone(&slots).acquire_owned().await else {
                    return;
                };
                let (sender, from) = (sender.clone(), from.clone());
                tokio::spawn(async move {
                    deliver_with_retries(&sender, &from, email).await;
                    drop(slot);
                });
            }
            Job::Flush(done) => {
                if let Ok(all) = slots.acquire_many(DELIVERIES).await {
                    drop(all);
                }
                let _ = done.send(());
            }
        }
    }
}

fn smtp(s: &SmtpSettings) -> Result<AsyncSmtpTransport<Tokio1Executor>, MailError> {
    let builder = match s.security {
        SmtpSecurity::Starttls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&s.host),
        SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&s.host),
        SmtpSecurity::None => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
            &s.host,
        )),
    }
    .map_err(|e| MailError::Config(e.to_string()))?;
    let mut builder = builder.port(s.port).timeout(Some(Duration::from_secs(20)));
    if let Some((user, password)) = &s.credentials {
        builder = builder.credentials(Credentials::new(
            user.clone(),
            password.expose_secret().to_owned(),
        ));
    }
    Ok(builder.build())
}

async fn deliver_with_retries(sender: &Sender, from: &Mailbox, email: Rendered) {
    let kind = email.kind.as_str();
    let mut attempt = 0;
    loop {
        match deliver(sender, from, &email).await {
            Ok(()) => {
                metrics::counter!("kaeso_emails_total", "kind" => kind, "outcome" => "sent")
                    .increment(1);
                return;
            }
            Err(MailError::Smtp(reason)) if attempt < RETRY_DELAYS.len() => {
                tracing::warn!(kind, attempt, error = %reason, "email delivery failed; retrying");
                if let Some(delay) = RETRY_DELAYS.get(attempt) {
                    tokio::time::sleep(*delay).await;
                }
                attempt += 1;
            }
            Err(e) => {
                metrics::counter!("kaeso_emails_total", "kind" => kind, "outcome" => "failed")
                    .increment(1);
                tracing::error!(kind, error = %e, "email could not be delivered");
                return;
            }
        }
    }
}

async fn deliver(sender: &Sender, from: &Mailbox, email: &Rendered) -> Result<(), MailError> {
    match sender {
        Sender::Log => {
            tracing::info!(
                kind = email.kind.as_str(),
                "email not sent (KAESO_MAIL_TRANSPORT=log)"
            );
            Ok(())
        }
        Sender::Memory(sink) => sink.push(email.clone()),
        Sender::Smtp(transport) => {
            let to: Mailbox = email
                .to
                .parse()
                .map_err(|_| MailError::Invalid("the recipient address".into()))?;
            let message = Message::builder()
                .from(from.clone())
                .to(to)
                .subject(email.subject.clone())
                .multipart(body(email)?)
                .map_err(|e| MailError::Invalid(e.to_string()))?;
            transport.send(message).await.map(|_| ()).map_err(|e| {
                if e.is_permanent() {
                    MailError::Refused(e.to_string())
                } else {
                    MailError::Smtp(e.to_string())
                }
            })
        }
    }
}

fn body(email: &Rendered) -> Result<MultiPart, MailError> {
    let png = ContentType::parse("image/png").map_err(|e| MailError::Invalid(e.to_string()))?;
    let mut related = MultiPart::related().singlepart(SinglePart::html(email.html.clone()));
    for image in assets::ALL {
        related = related.singlepart(
            Attachment::new_inline(image.cid.to_owned()).body(image.png.to_vec(), png.clone()),
        );
    }
    Ok(MultiPart::alternative()
        .singlepart(SinglePart::plain(email.text.clone()))
        .multipart(related))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_transport_receives_queued_emails() {
        let sink = MemorySink::new();
        let mailer = Mailer::start(
            Transport::Memory(sink.clone()),
            "Kaeso <kaeso@example.com>",
            Instance::new("https://kaeso.example.com/"),
        )
        .unwrap();
        mailer.send(crate::templates::test(
            crate::Locale::En,
            "owner@example.com",
        ));
        mailer.flush().await;
        assert_eq!(sink.to("OWNER@example.com").len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_email_does_not_hold_up_the_next_one() {
        let sink = MemorySink::new();
        sink.fail_next("first@example.com", 1);
        let mailer = Mailer::start(
            Transport::Memory(sink.clone()),
            "Kaeso <kaeso@example.com>",
            Instance::new("https://kaeso.example.com/"),
        )
        .unwrap();
        let started = tokio::time::Instant::now();
        for to in ["first@example.com", "second@example.com"] {
            mailer.send(crate::templates::test(crate::Locale::En, to));
        }
        mailer.flush().await;
        let order: Vec<String> = sink.all().into_iter().map(|e| e.to).collect();
        assert_eq!(order, ["second@example.com", "first@example.com"]);
        assert!(started.elapsed() >= RETRY_DELAYS[0]);
    }

    #[tokio::test(start_paused = true)]
    async fn flush_waits_for_emails_that_are_still_retrying() {
        let sink = MemorySink::new();
        sink.fail_next("owner@example.com", RETRY_DELAYS.len());
        let mailer = Mailer::start(
            Transport::Memory(sink.clone()),
            "Kaeso <kaeso@example.com>",
            Instance::new("https://kaeso.example.com/"),
        )
        .unwrap();
        mailer.send(crate::templates::test(
            crate::Locale::En,
            "owner@example.com",
        ));
        mailer.flush().await;
        assert_eq!(sink.to("owner@example.com").len(), 1);
    }

    #[test]
    fn a_bad_sender_address_is_a_config_error() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let _guard = rt.enter();
        assert!(matches!(
            Mailer::start(
                Transport::Log,
                "not an address",
                Instance::new("https://kaeso.example.com/")
            ),
            Err(MailError::Config(_))
        ));
    }
}
