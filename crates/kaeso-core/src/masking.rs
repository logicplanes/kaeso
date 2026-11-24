use std::collections::{BTreeMap, HashMap};

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

/// The most custom patterns an organization's rules may hold.
pub const MAX_CUSTOM: usize = 20;
/// The longest custom pattern, in characters.
pub const MAX_PATTERN: usize = 500;
/// How large a compiled custom pattern may grow; the `regex` crate matches in linear time, and
/// this keeps a pattern's program small.
const PATTERN_SIZE_LIMIT: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = GuardrailDetector))]
#[serde(rename_all = "snake_case")]
pub enum Detector {
    /// API keys of the providers Kaeso knows and of Kaeso, AWS access key ids, GitHub and GitLab
    /// tokens, private key PEM blocks and JSON Web Tokens.
    Secrets,
    /// Email addresses.
    Email,
    /// Phone numbers in E.164, such as `+4930123456`.
    Phone,
    /// IBANs whose mod-97 check holds.
    Iban,
    /// Payment card numbers whose Luhn check holds.
    Card,
    /// IPv4 and IPv6 addresses.
    Ip,
}

impl Detector {
    pub const ALL: [Detector; 6] = [
        Detector::Secrets,
        Detector::Email,
        Detector::Phone,
        Detector::Iban,
        Detector::Card,
        Detector::Ip,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Detector::Secrets => "secrets",
            Detector::Email => "email",
            Detector::Phone => "phone",
            Detector::Iban => "iban",
            Detector::Card => "card",
            Detector::Ip => "ip",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = GuardrailRuleAction))]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    /// Replace each value with a placeholder such as `[EMAIL_1]`.
    Mask,
    /// Refuse the request.
    Block,
    /// Let it through and count the values.
    Flag,
}

impl RuleAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            RuleAction::Mask => "mask",
            RuleAction::Block => "block",
            RuleAction::Flag => "flag",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = GuardrailPattern))]
pub struct CustomRule {
    /// 1 to 32 lowercase letters, digits and `_`, starting with a letter; the placeholder is
    /// the name in capitals, as in `[ORDER_ID_1]`.
    pub name: String,
    /// A regular expression in the `regex` crate's syntax, at most 500 characters.
    pub pattern: String,
    /// What a match does.
    pub action: RuleAction,
}

/// What built-in rules look for and what they do with it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = GuardrailRules))]
pub struct Rules {
    /// The detectors that run, with their action; a detector left out is off.
    #[serde(default)]
    pub detectors: BTreeMap<Detector, RuleAction>,
    /// The organization's own patterns.
    #[serde(default)]
    pub custom: Vec<CustomRule>,
    /// Mask every value found in the request log, also those the request sends unchanged.
    #[serde(default)]
    pub mask_log: bool,
    /// Put the original values back where the model's reply names their placeholders.
    #[serde(default)]
    pub restore: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RulesError {
    #[error("rules need a detector or a custom pattern")]
    Empty,
    #[error("at most {MAX_CUSTOM} custom patterns")]
    TooMany,
    #[error("custom pattern {0} has an invalid name")]
    Name(usize),
    #[error("custom pattern {0} repeats a name")]
    Repeated(usize),
    #[error("custom pattern {0} is empty or longer than {MAX_PATTERN} characters")]
    Length(usize),
    #[error("custom pattern {0} does not compile")]
    Pattern(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Check {
    None,
    Luhn,
    Iban,
    Ipv6,
}

#[derive(Debug)]
struct Finder {
    // What the log counts the values as, such as `api_key`.
    kind: String,
    // The placeholder's word, such as `API_KEY`.
    label: String,
    // The detector the action and the metrics belong to: a built-in one, or `custom`.
    detector: &'static str,
    action: RuleAction,
    regex: Regex,
    check: Check,
    // The capture group that holds the value: the phone pattern looks behind its match.
    group: usize,
}

/// Compiled rules, ready to scan text.
#[derive(Debug)]
pub struct Scanner {
    finders: Vec<Finder>,
    pub mask_log: bool,
    pub restore: bool,
}

const BUILT_IN_KINDS: &[&str] = &[
    "api_key",
    "private_key",
    "token",
    "email",
    "phone",
    "iban",
    "card",
    "ip",
];

fn key_prefixes() -> Vec<&'static str> {
    let mut prefixes: Vec<&'static str> = crate::provider::PROVIDERS
        .iter()
        .filter_map(|p| p.key_prefix)
        .chain([crate::keys::PROJECT_PREFIX])
        .collect();
    prefixes.sort_by_key(|p| std::cmp::Reverse(p.len()));
    prefixes.dedup();
    prefixes
}

fn built_in(detector: Detector) -> Vec<(&'static str, &'static str, String, Check)> {
    match detector {
        Detector::Secrets => {
            let prefixes = key_prefixes()
                .into_iter()
                .map(regex::escape)
                .collect::<Vec<_>>()
                .join("|");
            vec![
                (
                    "private_key",
                    "PRIVATE_KEY",
                    r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----"
                        .to_owned(),
                    Check::None,
                ),
                (
                    "token",
                    "TOKEN",
                    r"\beyJ[A-Za-z0-9_-]{5,}\.eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{10,}".to_owned(),
                    Check::None,
                ),
                (
                    "api_key",
                    "API_KEY",
                    format!(
                        r"(?:\b(?:{prefixes})[A-Za-z0-9_\-]{{16,}}|\b(?:AKIA|ASIA)[0-9A-Z]{{16}}\b|\b(?:gh[pousr]_[A-Za-z0-9]{{36,255}}|github_pat_[A-Za-z0-9_]{{22,255}})\b|\bglpat-[A-Za-z0-9_\-]{{20,}})"
                    ),
                    Check::None,
                ),
            ]
        }
        Detector::Email => vec![(
            "email",
            "EMAIL",
            r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(?:\.[A-Za-z0-9\-]+)*\.[A-Za-z]{2,}\b".to_owned(),
            Check::None,
        )],
        Detector::Phone => vec![(
            "phone",
            "PHONE",
            r"(?:^|[^0-9A-Za-z+])(\+[1-9][0-9]{7,14})\b".to_owned(),
            Check::None,
        )],
        Detector::Iban => vec![(
            "iban",
            "IBAN",
            r"\b[A-Z]{2}[0-9]{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,4})?\b".to_owned(),
            Check::Iban,
        )],
        Detector::Card => vec![(
            "card",
            "CARD",
            r"\b[0-9](?:[ \-]?[0-9]){12,18}\b".to_owned(),
            Check::Luhn,
        )],
        Detector::Ip => vec![
            (
                "ip",
                "IP",
                r"\b(?:(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\b"
                    .to_owned(),
                Check::None,
            ),
            (
                "ip",
                "IP",
                r"(?:[0-9A-Fa-f]{1,4}:){1,7}:?(?:[0-9A-Fa-f]{1,4}(?::[0-9A-Fa-f]{1,4}){0,6})?|::(?:[0-9A-Fa-f]{1,4}(?::[0-9A-Fa-f]{1,4}){0,6})"
                    .to_owned(),
                Check::Ipv6,
            ),
        ],
    }
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 32
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl Rules {
    /// Checks the rules and compiles them.
    pub fn compile(&self) -> Result<Scanner, RulesError> {
        if self.detectors.is_empty() && self.custom.is_empty() {
            return Err(RulesError::Empty);
        }
        if self.custom.len() > MAX_CUSTOM {
            return Err(RulesError::TooMany);
        }
        let mut finders = Vec::new();
        for (detector, action) in &self.detectors {
            for (kind, label, pattern, check) in built_in(*detector) {
                let regex = Regex::new(&pattern).map_err(|_| RulesError::Pattern(0))?;
                finders.push(Finder {
                    kind: kind.to_owned(),
                    label: label.to_owned(),
                    detector: detector.as_str(),
                    action: *action,
                    regex,
                    check,
                    group: usize::from(*detector == Detector::Phone),
                });
            }
        }
        for (i, rule) in self.custom.iter().enumerate() {
            if !valid_name(&rule.name) || BUILT_IN_KINDS.contains(&rule.name.as_str()) {
                return Err(RulesError::Name(i));
            }
            if self.custom.iter().take(i).any(|r| r.name == rule.name) {
                return Err(RulesError::Repeated(i));
            }
            if rule.pattern.is_empty() || rule.pattern.chars().count() > MAX_PATTERN {
                return Err(RulesError::Length(i));
            }
            let regex = RegexBuilder::new(&rule.pattern)
                .size_limit(PATTERN_SIZE_LIMIT)
                .build()
                .map_err(|_| RulesError::Pattern(i))?;
            finders.push(Finder {
                kind: rule.name.clone(),
                label: rule.name.to_ascii_uppercase(),
                detector: "custom",
                action: rule.action,
                regex,
                check: Check::None,
                group: 0,
            });
        }
        Ok(Scanner {
            finders,
            mask_log: self.mask_log,
            restore: self.restore,
        })
    }
}

fn luhn(digits: &str) -> bool {
    let digits: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| {
            if !i.is_multiple_of(2) {
                let x = d * 2;
                if x > 9 { x - 9 } else { x }
            } else {
                *d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn iban(text: &str) -> bool {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !(15..=34).contains(&compact.len()) {
        return false;
    }
    let (head, tail) = compact.split_at(4);
    let mut rem: u32 = 0;
    for c in tail.chars().chain(head.chars()) {
        let v = match c.to_digit(36) {
            Some(v) => v,
            None => return false,
        };
        rem = if v >= 10 {
            (rem * 100 + v) % 97
        } else {
            (rem * 10 + v) % 97
        };
    }
    rem == 1
}

fn passes(check: Check, text: &str) -> bool {
    match check {
        Check::None => true,
        Check::Luhn => luhn(text),
        Check::Iban => iban(text),
        Check::Ipv6 => {
            text.contains(':')
                && text.matches(':').count() >= 2
                && text.parse::<std::net::Ipv6Addr>().is_ok()
        }
    }
}

// How issuers print a card number in groups; a run of other groups that passes the checksum is
// chance, such as one in ten lists of single digits.
const CARD_GROUPS: [&[usize]; 4] = [&[4, 4, 4, 4], &[4, 4, 4, 4, 3], &[4, 6, 5], &[4, 6, 4]];

// A value matches together with the groups next to it, such as a card's security code, and
// then fails its checksum, so the value is looked for among the match's whole groups.
fn passing(check: Check, text: &str) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }
    if passes(check, text) {
        return Some((0, text.len()));
    }
    let separators: &[char] = match check {
        Check::Luhn => &[' ', '-'],
        Check::Iban => &[' '],
        Check::None | Check::Ipv6 => return None,
    };
    let mut groups = Vec::new();
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if separators.contains(&c) {
            groups.push((start, i));
            start = i + c.len_utf8();
        }
    }
    groups.push((start, text.len()));
    let run = |first: usize, len: usize| {
        let from = groups.get(first)?.0;
        let to = groups.get(first + len - 1)?.1;
        text.get(from..to)
            .is_some_and(|t| passes(check, t))
            .then_some((from, to))
    };
    if check == Check::Iban {
        // The number ends before a word: a shorter run of its own groups would pass by chance
        // once in 97 times.
        return (1..groups.len()).rev().find_map(|len| {
            groups
                .get(len)
                .and_then(|g| text.get(g.0..g.1))
                .is_some_and(|g| g.bytes().all(|b| b.is_ascii_uppercase()))
                .then(|| run(0, len))?
        });
    }
    let sizes: Vec<usize> = groups.iter().map(|g| g.1 - g.0).collect();
    for len in (1..groups.len()).rev() {
        let found = (0..=groups.len() - len)
            .filter(|first| {
                sizes
                    .get(*first..first + len)
                    .is_some_and(|run| len == 1 || CARD_GROUPS.contains(&run))
            })
            .find_map(|first| run(first, len));
        if found.is_some() {
            return found;
        }
    }
    None
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    start: usize,
    end: usize,
    finder: usize,
}

impl Scanner {
    fn hits(&self, text: &str) -> Vec<Hit> {
        let mut hits = Vec::new();
        for (i, f) in self.finders.iter().enumerate() {
            for caps in f.regex.captures_iter(text) {
                let Some(m) = caps.get(f.group) else {
                    continue;
                };
                if let Some((from, to)) = passing(f.check, m.as_str()) {
                    hits.push(Hit {
                        start: m.start() + from,
                        end: m.start() + to,
                        finder: i,
                    });
                }
            }
        }
        hits.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then(b.end.cmp(&a.end))
                .then(a.finder.cmp(&b.finder))
        });
        let mut kept: Vec<Hit> = Vec::new();
        for h in hits {
            if kept.last().is_none_or(|k| h.start >= k.end) {
                kept.push(h);
            }
        }
        kept
    }

    pub fn masker(&self) -> Masker<'_> {
        Masker {
            scanner: self,
            values: HashMap::new(),
            next: HashMap::new(),
            found: BTreeMap::new(),
            blocked: None,
            masked: Vec::new(),
            logged: Vec::new(),
        }
    }
}

/// How many values of each kind a scan found, and what was done with them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Found {
    pub count: u32,
    pub action: Option<RuleAction>,
    pub detector: String,
}

/// One request's scan: the same value keeps its placeholder throughout.
#[derive(Debug)]
pub struct Masker<'s> {
    scanner: &'s Scanner,
    values: HashMap<String, String>,
    next: HashMap<String, u32>,
    /// What each kind found, by kind, such as `api_key` or `email`.
    pub found: BTreeMap<String, Found>,
    /// The kind of the first value a blocking rule found.
    pub blocked: Option<String>,
    masked: Vec<(String, String)>,
    logged: Vec<(String, String)>,
}

impl Masker<'_> {
    fn placeholder(&mut self, finder: &Finder, value: &str) -> String {
        if let Some(p) = self.values.get(value) {
            return p.clone();
        }
        let n = self.next.entry(finder.kind.clone()).or_insert(0);
        *n += 1;
        let p = format!("[{}_{}]", finder.label, n);
        self.values.insert(value.to_owned(), p.clone());
        p
    }

    fn remember(list: &mut Vec<(String, String)>, value: &str, placeholder: &str) {
        if !list.iter().any(|(v, _)| v == value) {
            list.push((value.to_owned(), placeholder.to_owned()));
        }
    }

    /// Scans a text; returns it with masked values replaced, or `None` when nothing changes.
    pub fn text(&mut self, text: &str) -> Option<String> {
        let hits = self.scanner.hits(text);
        if hits.is_empty() {
            return None;
        }
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        let mut changed = false;
        for h in hits {
            let Some(finder) = self.scanner.finders.get(h.finder) else {
                continue;
            };
            let value = text.get(h.start..h.end).unwrap_or_default();
            let entry = self
                .found
                .entry(finder.kind.clone())
                .or_insert_with(|| Found {
                    count: 0,
                    action: Some(finder.action),
                    detector: finder.detector.to_owned(),
                });
            entry.count += 1;
            match finder.action {
                RuleAction::Block => {
                    self.blocked.get_or_insert_with(|| finder.kind.clone());
                    if self.scanner.mask_log {
                        let p = self.placeholder(finder, value);
                        Self::remember(&mut self.logged, value, &p);
                    }
                }
                RuleAction::Flag if self.scanner.mask_log => {
                    let p = self.placeholder(finder, value);
                    Self::remember(&mut self.logged, value, &p);
                }
                RuleAction::Flag => {}
                RuleAction::Mask => {
                    let p = self.placeholder(finder, value);
                    Self::remember(&mut self.masked, value, &p);
                    Self::remember(&mut self.logged, value, &p);
                    out.push_str(text.get(at..h.start).unwrap_or_default());
                    out.push_str(&p);
                    at = h.end;
                    changed = true;
                }
            }
        }
        if !changed {
            return None;
        }
        out.push_str(text.get(at..).unwrap_or_default());
        Some(out)
    }

    /// The values the request log replaces, each with its placeholder.
    pub fn logged(&self) -> &[(String, String)] {
        &self.logged
    }

    /// What puts the masked values back into a reply, when the rules ask for it.
    pub fn restorer(&self) -> Option<Restorer> {
        (self.scanner.restore && !self.masked.is_empty()).then(|| Restorer::new(&self.masked))
    }
}

/// Puts masked values back where a text names their placeholders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Restorer {
    pairs: Vec<(String, String)>,
    longest: usize,
}

impl Restorer {
    fn new(masked: &[(String, String)]) -> Restorer {
        let pairs: Vec<(String, String)> =
            masked.iter().map(|(v, p)| (p.clone(), v.clone())).collect();
        let longest = pairs.iter().map(|(p, _)| p.len()).max().unwrap_or(0);
        Restorer { pairs, longest }
    }

    pub fn restore(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (p, v) in &self.pairs {
            if out.contains(p.as_str()) {
                out = out.replace(p.as_str(), v);
            }
        }
        out
    }

    /// Adds another scan's placeholders.
    pub fn extend(&mut self, other: &Restorer) {
        for pair in &other.pairs {
            if !self.pairs.contains(pair) {
                self.pairs.push(pair.clone());
            }
        }
        self.longest = self.longest.max(other.longest);
    }

    /// The longest placeholder; a stream holds back at most this many bytes less one.
    pub fn longest(&self) -> usize {
        self.longest
    }

    pub fn stream(&self) -> StreamRestore {
        StreamRestore {
            restorer: self.clone(),
            held: String::new(),
        }
    }
}

/// Restores placeholders in a text that arrives in pieces, holding back the end of a piece that
/// may be the start of a placeholder.
#[derive(Clone, Debug)]
pub struct StreamRestore {
    restorer: Restorer,
    held: String,
}

fn could_start_placeholder(tail: &str) -> bool {
    tail.starts_with('[')
        && tail
            .chars()
            .skip(1)
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

impl StreamRestore {
    pub fn push(&mut self, piece: &str) -> String {
        self.held.push_str(piece);
        let keep_from = self
            .held
            .rfind('[')
            .filter(|i| {
                let tail = self.held.get(*i..).unwrap_or_default();
                tail.len() < self.restorer.longest && could_start_placeholder(tail)
            })
            .unwrap_or(self.held.len());
        let ready: String = self.held.drain(..keep_from).collect();
        self.restorer.restore(&ready)
    }

    pub fn finish(&mut self) -> String {
        let rest = std::mem::take(&mut self.held);
        self.restorer.restore(&rest)
    }
}

/// Replaces each value in a text with its placeholder, also as JSON writes it inside a string.
pub fn mask_text(text: &str, pairs: &[(String, String)]) -> String {
    let mut out = text.to_owned();
    for (value, placeholder) in pairs {
        if out.contains(value.as_str()) {
            out = out.replace(value.as_str(), placeholder);
        }
        if let Ok(json) = serde_json::to_string(value)
            && let Some(inner) = json.get(1..json.len().saturating_sub(1))
            && inner != value
            && out.contains(inner)
        {
            out = out.replace(inner, placeholder);
        }
    }
    out
}

#[cfg(test)]
#[path = "masking_tests.rs"]
mod tests;
