use std::collections::HashMap;

use kaeso_core::rights::Right;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, OffsetDateTime, Time};

use crate::services::agents::{MODELS, Usable};

pub const TOOL: &str = "show_usage_stats";
pub const VIEW: &str = "ui://kaeso/stats";
// The MIME type of an MCP App's HTML resource, from the MCP Apps extension (SEP-1865):
// https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx#ui-resource-format
pub const MIME: &str = "text/html;profile=mcp-app";
pub const HTML: &str = include_str!("../../assets/mcp-stats.html");
pub const TOP: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    Requests,
    Tokens,
    Cost,
    Latency,
    Errors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    Day,
    Week,
    Month,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Model,
    Provider,
    Key,
}

impl Metric {
    const ALL: [(&'static str, Metric); 5] = [
        ("requests", Metric::Requests),
        ("tokens", Metric::Tokens),
        ("cost", Metric::Cost),
        ("latency", Metric::Latency),
        ("errors", Metric::Errors),
    ];
    fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, m)| *m == self)
            .map_or("requests", |(n, _)| n)
    }
}

impl Range {
    const ALL: [(&'static str, Range); 3] = [
        ("24h", Range::Day),
        ("7d", Range::Week),
        ("30d", Range::Month),
    ];
    fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, r)| *r == self)
            .map_or("7d", |(n, _)| n)
    }
    fn days(self) -> i64 {
        match self {
            Range::Day => 1,
            Range::Week => 7,
            Range::Month => 30,
        }
    }
}

impl Group {
    const ALL: [(&'static str, Group); 3] = [
        ("model", Group::Model),
        ("provider", Group::Provider),
        ("key", Group::Key),
    ];
    fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, g)| *g == self)
            .map_or("model", |(n, _)| n)
    }
}

fn names<T>(all: &[(&'static str, T)]) -> Vec<&'static str> {
    all.iter().map(|(n, _)| *n).collect()
}

pub fn usable(usable: &Usable) -> bool {
    ["usage.org.view", "usage.view"]
        .iter()
        .filter_map(|r| r.parse::<Right>().ok())
        .any(|r| usable.contains(r))
}

pub fn listing() -> Value {
    json!({
        "name": TOOL,
        "title": "Show usage stats",
        "description": "Shows an organization's or a project's usage over the last 24 hours, 7 days or 30 days as an interactive chart with its totals, broken down by model, provider or key. Hosts that support MCP Apps draw the view; others get a short summary. Costs are USD strings, `null` without the right to see costs; latency is the 95th percentile of the slowest hour.\n\n**Right:** `usage.org.view` for an organization, `usage.view` for a project.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "metric": { "type": "string", "enum": names(&Metric::ALL), "description": "What the chart draws; `requests` by default." },
                "range": { "type": "string", "enum": names(&Range::ALL), "description": "The period, ending now; `7d` by default." },
                "group_by": { "type": "string", "enum": names(&Group::ALL), "description": "What the breakdown under the chart is by; `model` by default." },
                "org": { "type": "string", "description": "The organization (`org_…`); needed when the user belongs to more than one." },
                "project": { "type": "string", "description": "One project (`proj_…`) of the organization, instead of all of them." },
                "agent_model": { "type": "string", "enum": MODELS, "description": "The model family you are; `unknown` if none of these fits. Kaeso shows it next to what you do." },
            },
            "additionalProperties": false,
        },
        "annotations": {
            "title": "Show usage stats",
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false,
        },
        "_meta": {
            "ui": { "resourceUri": VIEW },
            "ui/resourceUri": VIEW,
        },
    })
}

// An empty list in each field is the strictest policy the extension allows: no connections,
// no outside scripts, styles, images or fonts, no frames and no other base URI.
pub fn ui_meta() -> Value {
    json!({
        "csp": { "connectDomains": [], "resourceDomains": [], "frameDomains": [], "baseUriDomains": [] },
        "prefersBorder": true,
    })
}

pub fn resource() -> Value {
    json!({
        "uri": VIEW,
        "name": "kaeso_stats",
        "title": "Usage stats",
        "description": "The interactive chart and totals that show_usage_stats draws.",
        "mimeType": MIME,
        "_meta": { "ui": ui_meta() },
    })
}

pub fn contents() -> Value {
    let mut meta = ui_meta();
    // Claude's own field for opening a view in its side panel without a click:
    // https://claude.com/docs/connectors/building/mcp-apps/display-modes#open-a-view-in-the-side-panel-without-a-click
    if let Some(m) = meta.as_object_mut() {
        m.insert("preferredDisplayMode".to_owned(), json!("fullscreen"));
    }
    json!({
        "contents": [{
            "uri": VIEW,
            "mimeType": MIME,
            "text": HTML,
            "_meta": { "ui": meta },
        }]
    })
}

impl Ask {
    pub fn group_name(&self) -> &'static str {
        self.group.name()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Ask {
    pub metric: Metric,
    pub range: Range,
    pub group: Group,
    pub org: Option<String>,
    pub project: Option<String>,
}

pub fn ask(args: &Map<String, Value>) -> Result<Ask, String> {
    fn pick<T: Copy>(
        args: &Map<String, Value>,
        key: &str,
        all: &[(&'static str, T)],
        default: T,
    ) -> Result<T, String> {
        match args.get(key) {
            None | Some(Value::Null) => Ok(default),
            Some(Value::String(s)) => all
                .iter()
                .find(|(n, _)| n == s)
                .map(|(_, v)| *v)
                .ok_or_else(|| format!("`{key}` must be one of {}", names(all).join(", "))),
            Some(_) => Err(format!("`{key}` must be a string")),
        }
    }
    fn id(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
        match args.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
            Some(_) => Err(format!("`{key}` must be an id")),
        }
    }
    for key in args.keys() {
        if ![
            "metric",
            "range",
            "group_by",
            "org",
            "project",
            "agent_model",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("`{key}` is not an argument of this tool"));
        }
    }
    Ok(Ask {
        metric: pick(args, "metric", &Metric::ALL, Metric::Requests)?,
        range: pick(args, "range", &Range::ALL, Range::Week)?,
        group: pick(args, "group_by", &Group::ALL, Group::Model)?,
        org: id(args, "org")?,
        project: id(args, "project")?,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct Window {
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
    pub first_day: Date,
    pub last_day: Date,
}

pub fn window(range: Range, now: OffsetDateTime) -> Window {
    let now = now.to_offset(time::UtcOffset::UTC);
    let hour = now.replace_time(Time::from_hms(now.hour(), 0, 0).unwrap_or(Time::MIDNIGHT));
    let last_day = now.date();
    let first_day = last_day - Duration::days(range.days() - 1);
    let start = match range {
        Range::Day => hour - Duration::hours(23),
        _ => first_day.midnight().assume_utc(),
    };
    Window {
        start,
        end: now,
        first_day,
        last_day,
    }
}

pub fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

pub fn day(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())
}

fn int(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn tokens(v: &Value) -> i64 {
    [
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cache_write_tokens",
    ]
    .iter()
    .map(|k| int(v, k))
    .sum()
}

fn cost(v: &Value) -> Option<Decimal> {
    v.get("cost")
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
}

fn sum_cost(rows: &[&Value]) -> Option<Decimal> {
    if !rows.is_empty() && rows.iter().all(|r| cost(r).is_none()) {
        return None;
    }
    Some(rows.iter().filter_map(|r| cost(r)).sum())
}

fn busy<'a>(rows: &[&'a Value]) -> Vec<&'a Value> {
    rows.iter()
        .copied()
        .filter(|r| int(r, "requests") > 0)
        .collect()
}

fn slowest(rows: &[&Value]) -> Option<i64> {
    busy(rows).iter().map(|r| int(r, "duration_p95_ms")).max()
}

fn median_hour(rows: &[&Value]) -> Option<i64> {
    let mut p50: Vec<i64> = busy(rows)
        .iter()
        .map(|r| int(r, "duration_p50_ms"))
        .collect();
    p50.sort_unstable();
    p50.get(p50.len() / 2).copied()
}

fn money(d: Decimal) -> Value {
    json!(d.normalize().to_string())
}

fn value_of(metric: Metric, rows: &[&Value]) -> Value {
    match metric {
        Metric::Requests => json!(rows.iter().map(|r| int(r, "requests")).sum::<i64>()),
        Metric::Errors => json!(rows.iter().map(|r| int(r, "errors")).sum::<i64>()),
        Metric::Tokens => json!(rows.iter().map(|r| tokens(r)).sum::<i64>()),
        Metric::Cost => sum_cost(rows).map_or(Value::Null, money),
        Metric::Latency => slowest(rows).map_or(Value::Null, |v| json!(v)),
    }
}

fn rank(v: &Value) -> Decimal {
    match v {
        Value::Number(n) => n.as_i64().map(Decimal::from).unwrap_or_default(),
        Value::String(s) => s.parse().unwrap_or_default(),
        _ => Decimal::MIN,
    }
}

fn started(v: &Value) -> Option<i64> {
    v.get("start")
        .and_then(Value::as_str)
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok())
        .map(OffsetDateTime::unix_timestamp)
}

pub struct Data<'a> {
    pub hours: &'a [Value],
    pub days: &'a [Value],
    pub grouped: &'a [Value],
    pub key_names: &'a HashMap<String, String>,
}

pub fn totals(ask: &Ask, data: &Data<'_>) -> Value {
    let hours: Vec<&Value> = data.hours.iter().collect();
    let days: Vec<&Value> = data.days.iter().collect();
    let sums = if ask.range == Range::Day {
        &hours
    } else {
        &days
    };
    json!({
        "requests": value_of(Metric::Requests, sums),
        "tokens": value_of(Metric::Tokens, sums),
        "cost": value_of(Metric::Cost, sums),
        "errors": value_of(Metric::Errors, sums),
        "latency_p95_ms": slowest(&hours),
        "latency_p50_ms": median_hour(&hours),
    })
}

pub fn series(ask: &Ask, w: &Window, data: &Data<'_>) -> (&'static str, Vec<Value>) {
    if ask.range == Range::Day || ask.metric == Metric::Latency {
        let mut by_hour: HashMap<i64, Vec<&Value>> = HashMap::new();
        for b in data.hours {
            if let Some(t) = started(b) {
                by_hour.entry(t).or_default().push(b);
            }
        }
        let mut out = Vec::new();
        let mut at = w.start;
        while at <= w.end {
            let rows = by_hour
                .get(&at.unix_timestamp())
                .cloned()
                .unwrap_or_default();
            out.push(json!({ "start": rfc3339(at), "value": value_of(ask.metric, &rows) }));
            at += Duration::hours(1);
        }
        return ("hour", out);
    }
    let by_day: HashMap<&str, &Value> = data
        .days
        .iter()
        .filter_map(|r| r.get("key").and_then(Value::as_str).map(|k| (k, r)))
        .collect();
    let mut out = Vec::new();
    let mut d = w.first_day;
    while d <= w.last_day {
        let key = day(d);
        let rows: Vec<&Value> = by_day.get(key.as_str()).copied().into_iter().collect();
        out.push(json!({ "start": key, "value": value_of(ask.metric, &rows) }));
        match d.next_day() {
            Some(n) => d = n,
            None => break,
        }
    }
    ("day", out)
}

pub fn groups(ask: &Ask, data: &Data<'_>) -> Vec<Value> {
    let mut by_key: Vec<(String, Vec<&Value>)> = Vec::new();
    for r in data.grouped {
        let Some(key) = r.get("key").and_then(Value::as_str) else {
            continue;
        };
        match by_key.iter_mut().find(|(k, _)| k == key) {
            Some((_, rows)) => rows.push(r),
            None => by_key.push((key.to_owned(), vec![r])),
        }
    }
    let mut out: Vec<(Decimal, String, Value)> = by_key
        .into_iter()
        .map(|(key, rows)| {
            let value = value_of(ask.metric, &rows);
            let label = data
                .key_names
                .get(&key)
                .cloned()
                .unwrap_or_else(|| key.clone());
            (
                rank(&value),
                key.clone(),
                json!({
                    "key": key,
                    "label": label,
                    "value": value,
                    "requests": rows.iter().map(|r| int(r, "requests")).sum::<i64>(),
                }),
            )
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    out.into_iter().take(TOP).map(|(_, _, v)| v).collect()
}

fn shown(metric: Metric, v: &Value) -> String {
    match (metric, v) {
        (_, Value::Null) => "not visible to you".to_owned(),
        (Metric::Cost, Value::String(s)) => {
            let d: Decimal = s.parse().unwrap_or_default();
            format!("${:.2}", d.round_dp(2))
        }
        (Metric::Latency, v) => format!("{v} ms"),
        (_, v) => v.to_string(),
    }
}

pub fn summary(ask: &Ask, scope: &str, totals: &Value, groups: &[Value]) -> String {
    let metric = ask.metric;
    let key = match metric {
        Metric::Latency => "latency_p95_ms",
        m => m.name(),
    };
    let what = match metric {
        Metric::Latency => "Latency (95th percentile of the slowest hour)".to_owned(),
        m => {
            let n = m.name();
            let mut c = n.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        }
    };
    let mut text = format!(
        "{what} of {scope} in the last {}: {}. {} requests, {} failed.",
        match ask.range {
            Range::Day => "24 hours",
            Range::Week => "7 days",
            Range::Month => "30 days",
        },
        shown(metric, &totals[key]),
        totals["requests"],
        totals["errors"],
    );
    let top: Vec<String> = groups
        .iter()
        .take(3)
        .map(|g| {
            format!(
                "{} ({})",
                g["label"].as_str().unwrap_or_default(),
                shown(metric, &g["value"])
            )
        })
        .collect();
    if !top.is_empty() {
        text.push_str(&format!(
            " Top by {}: {}.",
            ask.group.name(),
            top.join(", ")
        ));
    }
    text
}

pub fn structured(
    ask: &Ask,
    w: &Window,
    data: &Data<'_>,
    scope: Value,
    console_url: String,
) -> (Value, Vec<Value>, Value) {
    let totals = totals(ask, data);
    let (step, points) = series(ask, w, data);
    let groups = groups(ask, data);
    let out = json!({
        "metric": ask.metric.name(),
        "range": ask.range.name(),
        "group_by": ask.group.name(),
        "scope": scope,
        "from": rfc3339(w.start),
        "to": rfc3339(w.end),
        "step": step,
        "totals": totals,
        "series": points,
        "groups": groups,
        "console_url": console_url,
    });
    (out, groups, totals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn args(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn arguments_have_defaults_and_refuse_strangers() {
        let a = ask(&args(json!({}))).unwrap();
        assert_eq!(
            a,
            Ask {
                metric: Metric::Requests,
                range: Range::Week,
                group: Group::Model,
                org: None,
                project: None
            }
        );
        let a = ask(&args(json!({ "metric": "cost", "range": "24h", "group_by": "key", "org": "org_1", "agent_model": "claude" }))).unwrap();
        assert_eq!(
            (a.metric, a.range, a.group),
            (Metric::Cost, Range::Day, Group::Key)
        );
        assert_eq!(a.org.as_deref(), Some("org_1"));
        assert!(
            ask(&args(json!({ "metric": "money" })))
                .unwrap_err()
                .contains("requests, tokens, cost, latency, errors")
        );
        assert!(ask(&args(json!({ "range": 7 }))).is_err());
        assert!(
            ask(&args(json!({ "days": 7 })))
                .unwrap_err()
                .contains("`days`")
        );
    }

    #[test]
    fn the_listing_links_the_view_both_ways_and_takes_only_its_inputs() {
        let l = listing();
        assert_eq!(l["_meta"]["ui"]["resourceUri"], VIEW);
        assert_eq!(l["_meta"]["ui/resourceUri"], VIEW);
        assert_eq!(l["annotations"]["readOnlyHint"], true);
        let props = l["inputSchema"]["properties"].as_object().unwrap();
        let mut keys: Vec<&str> = props.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "agent_model",
                "group_by",
                "metric",
                "org",
                "project",
                "range"
            ]
        );
        assert_eq!(
            props["metric"]["enum"],
            json!(["requests", "tokens", "cost", "latency", "errors"])
        );
        assert_eq!(props["range"]["enum"], json!(["24h", "7d", "30d"]));
        assert_eq!(
            props["group_by"]["enum"],
            json!(["model", "provider", "key"])
        );
        assert!(l["inputSchema"].get("required").is_none());
    }

    #[test]
    fn the_resource_is_one_html_document_under_a_strict_policy() {
        let c = contents();
        let item = &c["contents"][0];
        assert_eq!(item["uri"], VIEW);
        assert_eq!(item["mimeType"], "text/html;profile=mcp-app");
        let csp = &item["_meta"]["ui"]["csp"];
        for field in [
            "connectDomains",
            "resourceDomains",
            "frameDomains",
            "baseUriDomains",
        ] {
            assert_eq!(csp[field], json!([]), "{field}");
        }
        assert_eq!(item["_meta"]["ui"]["prefersBorder"], true);
        assert_eq!(resource()["_meta"]["ui"], ui_meta());
        let html = item["text"].as_str().unwrap();
        assert!(html.starts_with("<!doctype html>"), "{}", &html[..40]);
        assert!(html.contains(r#"<meta name="color-scheme" content="light dark" />"#));
        for outside in [
            "<script src",
            "<link ",
            "src=\"http",
            "href=\"http",
            "url(http",
            "@import",
            "eval(",
        ] {
            assert!(!html.contains(outside), "the view loads {outside}");
        }
    }

    #[test]
    fn the_inline_script_is_the_only_one_its_policy_allows() {
        use base64::Engine as _;
        use sha2::Digest as _;
        let open = r#"<script type="module">"#;
        assert_eq!(HTML.matches(open).count(), 1);
        let start = HTML.find(open).unwrap() + open.len();
        let end = start + HTML[start..].find("</script>").unwrap();
        let hash = base64::engine::general_purpose::STANDARD
            .encode(sha2::Sha256::digest(&HTML.as_bytes()[start..end]));
        let policy = format!("script-src 'sha256-{hash}'");
        assert!(
            HTML.contains(&policy),
            "the policy names another script than {hash}"
        );
        for directive in [
            "default-src 'none'",
            "connect-src 'none'",
            "frame-src 'none'",
            "object-src 'none'",
            "base-uri 'none'",
            "form-action 'none'",
        ] {
            assert!(HTML.contains(directive), "{directive}");
        }
        assert!(!HTML.contains("unsafe-eval"));
    }

    #[test]
    fn windows_end_now_and_start_on_the_hour_or_the_day() {
        let now = datetime!(2026-11-10 13:27:05 UTC);
        let d = window(Range::Day, now);
        assert_eq!(rfc3339(d.start), "2026-11-09T14:00:00Z");
        let w = window(Range::Week, now);
        assert_eq!(rfc3339(w.start), "2026-11-04T00:00:00Z");
        assert_eq!(
            (day(w.first_day), day(w.last_day)),
            ("2026-11-04".to_owned(), "2026-11-10".to_owned())
        );
        assert_eq!(day(window(Range::Month, now).first_day), "2026-10-12");
    }

    fn hour(start: &str, requests: i64, p50: i64, p95: i64, cost: Option<&str>) -> Value {
        json!({ "start": start, "requests": requests, "errors": 1, "input_tokens": 10, "output_tokens": 5, "cache_read_tokens": 0, "cache_write_tokens": 0, "duration_p50_ms": p50, "duration_p95_ms": p95, "cost": cost })
    }

    #[test]
    fn a_day_draws_every_hour_and_sums_them_exactly() {
        let now = datetime!(2026-11-10 13:27:05 UTC);
        let w = window(Range::Day, now);
        let hours = vec![
            hour("2026-11-10T12:00:00Z", 3, 100, 900, Some("0.10")),
            hour("2026-11-10T13:00:00Z", 2, 300, 400, Some("0.20")),
            hour("2026-11-10T11:00:00Z", 0, 0, 0, None),
        ];
        let grouped = vec![
            json!({ "key": "openai/gpt-5", "requests": 4, "errors": 0, "cost": "0.25", "input_tokens": 1, "output_tokens": 1, "cache_read_tokens": 0, "cache_write_tokens": 0 }),
            json!({ "key": "anthropic/claude", "requests": 1, "errors": 0, "cost": "0.05", "input_tokens": 1, "output_tokens": 1, "cache_read_tokens": 0, "cache_write_tokens": 0 }),
        ];
        let names = HashMap::new();
        let data = Data {
            hours: &hours,
            days: &[],
            grouped: &grouped,
            key_names: &names,
        };
        let a = Ask {
            metric: Metric::Cost,
            range: Range::Day,
            group: Group::Model,
            org: None,
            project: None,
        };
        let t = totals(&a, &data);
        assert_eq!(t["cost"], "0.3");
        assert_eq!(t["requests"], 5);
        assert_eq!(t["tokens"], 45);
        assert_eq!(t["latency_p95_ms"], 900);
        assert_eq!(t["latency_p50_ms"], 300);
        let (step, points) = series(&a, &w, &data);
        assert_eq!(step, "hour");
        assert_eq!(points.len(), 24);
        assert_eq!(
            points[22],
            json!({ "start": "2026-11-10T12:00:00Z", "value": "0.1" })
        );
        assert_eq!(points[0]["value"], "0");
        let g = groups(&a, &data);
        assert_eq!(g[0]["key"], "openai/gpt-5");
        assert_eq!(g[1]["value"], "0.05");
        let text = summary(&a, "Acme", &t, &g);
        assert_eq!(
            text,
            "Cost of Acme in the last 24 hours: $0.30. 5 requests, 3 failed. Top by model: openai/gpt-5 ($0.25), anthropic/claude ($0.05)."
        );
    }

    #[test]
    fn days_fill_the_gaps_and_hidden_costs_stay_hidden() {
        let now = datetime!(2026-11-10 08:00:00 UTC);
        let w = window(Range::Week, now);
        let days = vec![
            json!({ "key": "2026-11-05", "requests": 7, "errors": 0, "cost": null, "input_tokens": 1, "output_tokens": 1, "cache_read_tokens": 0, "cache_write_tokens": 0 }),
        ];
        let names = HashMap::from([("key_1".to_owned(), "ci-runner".to_owned())]);
        let grouped = vec![json!({ "key": "key_1", "requests": 7, "errors": 0, "cost": null })];
        let data = Data {
            hours: &[],
            days: &days,
            grouped: &grouped,
            key_names: &names,
        };
        let a = Ask {
            metric: Metric::Cost,
            range: Range::Week,
            group: Group::Key,
            org: None,
            project: None,
        };
        let (step, points) = series(&a, &w, &data);
        assert_eq!(step, "day");
        assert_eq!(points.len(), 7);
        assert_eq!(points[1], json!({ "start": "2026-11-05", "value": null }));
        assert_eq!(points[0], json!({ "start": "2026-11-04", "value": "0" }));
        assert_eq!(totals(&a, &data)["cost"], Value::Null);
        assert_eq!(totals(&a, &data)["latency_p95_ms"], Value::Null);
        let g = groups(&a, &data);
        assert_eq!(g[0]["label"], "ci-runner");
        assert!(summary(&a, "Acme", &totals(&a, &data), &g).contains("not visible to you"));
    }
}
