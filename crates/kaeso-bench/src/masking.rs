use std::time::Instant;

use kaeso_core::masking::{Detector, RuleAction, Rules};
use kaeso_kpp::{Block, GenerateRequest, Message, Role};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MaskingResult {
    pub(crate) megabytes: usize,
    pub(crate) seconds: f64,
    pub(crate) megabytes_per_second: f64,
    pub(crate) found: u64,
    pub(crate) planted: u64,
}

// A conversation of about `megabytes` MB of agent-like text with a value to mask every few
// hundred bytes, scanned by every detector, as a request with built-in rules is. Returns the
// request and how many values it holds: four a line (an email, a phone number, a token, an IP).
fn conversation(megabytes: usize) -> (GenerateRequest, u64) {
    let line = "The build failed on ci-runner-7 again; mail ada@example.com or call +4930123456, \
                and use the token sk-kso-0123456789abcdef0123456789abcdef from 192.0.2.7 today.\n";
    let per_message = 64 * 1024;
    let lines_per_message = per_message / line.len();
    let text = line.repeat(lines_per_message);
    let messages = (megabytes * 1024 * 1024) / text.len();
    let mut g = GenerateRequest::new("bench");
    for i in 0..messages {
        let role = if i % 2 == 0 {
            Role::User
        } else {
            Role::Assistant
        };
        g = g.with_message(Message::new(role, vec![Block::text(text.clone())]));
    }
    (g, (messages * lines_per_message * 4) as u64)
}

pub(crate) async fn run(megabytes: usize) -> MaskingResult {
    let rules = Rules {
        detectors: Detector::ALL
            .into_iter()
            .map(|d| (d, RuleAction::Mask))
            .collect(),
        mask_log: true,
        restore: true,
        ..Rules::default()
    };
    let result = tokio::task::spawn_blocking(move || {
        let scanner = rules.compile().ok()?;
        let (mut g, planted) = conversation(megabytes);
        let started = Instant::now();
        let masked = kaeso_server::services::guardrails::rules::mask_request(&scanner, &mut g);
        let seconds = started.elapsed().as_secs_f64();
        let found: u64 = masked.found.values().map(|n| u64::from(*n)).sum();
        Some((seconds, found, planted))
    })
    .await
    .ok()
    .flatten();
    let (seconds, found, planted) = result.unwrap_or((f64::INFINITY, 0, 1));
    MaskingResult {
        megabytes,
        seconds,
        megabytes_per_second: megabytes as f64 / seconds,
        found,
        planted,
    }
}
