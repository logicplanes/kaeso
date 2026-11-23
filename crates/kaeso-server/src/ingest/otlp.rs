//! OTLP/HTTP request bodies (opentelemetry-proto v1.x, `opentelemetry/proto/collector/*/v1`),
//! as protobuf and as the JSON mapping the OTLP specification defines: field names in
//! lowerCamelCase, 64-bit integers as strings or numbers, ids as hex. Only what Kaeso reads is
//! decoded; protobuf skips unknown fields and JSON ignores them.

use serde::Deserialize;
use serde_json::Value as Json;

/// An attribute value, whichever encoding carried it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Bool(bool),
    Int(i64),
    Double(f64),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Map(Vec<(String, Value)>),
    Empty,
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The value as a whole number: integers, whole doubles and numeric strings, as clients
    /// write counts and durations either way.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Int(i) => u64::try_from(*i).ok(),
            Value::Double(d) if d.is_finite() && *d >= 0.0 && d.fract() == 0.0 => {
                format!("{d:.0}").parse().ok()
            }
            Value::Str(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    /// A decimal number written exactly as it came, never through a float.
    pub fn as_decimal_text(&self) -> Option<String> {
        match self {
            Value::Int(i) => Some(i.to_string()),
            Value::Double(d) if d.is_finite() => Some(format!("{d}")),
            Value::Str(s) => Some(s.trim().to_owned()),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Str(s) => match s.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    /// Text for a value meant to be shown, such as a list of finish reasons.
    pub fn to_text(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Double(d) => d.to_string(),
            Value::Bytes(_) | Value::Empty => String::new(),
            Value::Array(items) => items
                .iter()
                .map(Value::to_text)
                .collect::<Vec<_>>()
                .join(","),
            Value::Map(_) => String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attrs(pub Vec<(String, Value)>);

impl Attrs {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    pub fn u64(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(Value::as_u64)
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogRecord {
    pub time_unix_nano: u64,
    pub observed_time_unix_nano: u64,
    pub body: Option<Value>,
    pub attributes: Attrs,
    pub event_name: String,
    /// The span the record was logged in, so a record and its span name the same call.
    pub trace_id: Vec<u8>,
    pub span_id: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub trace_id: Vec<u8>,
    pub span_id: Vec<u8>,
    pub name: String,
    pub start_time_unix_nano: u64,
    pub end_time_unix_nano: u64,
    pub attributes: Attrs,
    /// 0 unset, 1 ok, 2 error.
    pub status_code: i32,
    pub status_message: String,
}

/// Records of one resource and scope, as the exporter grouped them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group<T> {
    pub resource: Attrs,
    pub scope: String,
    pub items: Vec<T>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("the body is not an OTLP protobuf message: {0}")]
    Protobuf(String),
    #[error("the body is not OTLP JSON: {0}")]
    Json(String),
    #[error("the export holds more records than a batch may")]
    TooMany,
}

mod pb {
    #![allow(clippy::derive_partial_eq_without_eq)]

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct AnyValue {
        #[prost(oneof = "Kind", tags = "1, 2, 3, 4, 5, 6, 7")]
        pub(super) value: Option<Kind>,
    }

    #[derive(Clone, PartialEq, prost::Oneof)]
    pub(super) enum Kind {
        #[prost(string, tag = "1")]
        String(String),
        #[prost(bool, tag = "2")]
        Bool(bool),
        #[prost(int64, tag = "3")]
        Int(i64),
        #[prost(double, tag = "4")]
        Double(f64),
        #[prost(message, tag = "5")]
        Array(ArrayValue),
        #[prost(message, tag = "6")]
        Kvlist(KeyValueList),
        #[prost(bytes, tag = "7")]
        Bytes(Vec<u8>),
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ArrayValue {
        #[prost(message, repeated, tag = "1")]
        pub(super) values: Vec<AnyValue>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct KeyValueList {
        #[prost(message, repeated, tag = "1")]
        pub(super) values: Vec<KeyValue>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct KeyValue {
        #[prost(string, tag = "1")]
        pub(super) key: String,
        #[prost(message, optional, tag = "2")]
        pub(super) value: Option<AnyValue>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct Resource {
        #[prost(message, repeated, tag = "1")]
        pub(super) attributes: Vec<KeyValue>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct InstrumentationScope {
        #[prost(string, tag = "1")]
        pub(super) name: String,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ExportLogsServiceRequest {
        #[prost(message, repeated, tag = "1")]
        pub(super) resource_logs: Vec<ResourceLogs>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ResourceLogs {
        #[prost(message, optional, tag = "1")]
        pub(super) resource: Option<Resource>,
        #[prost(message, repeated, tag = "2")]
        pub(super) scope_logs: Vec<ScopeLogs>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ScopeLogs {
        #[prost(message, optional, tag = "1")]
        pub(super) scope: Option<InstrumentationScope>,
        #[prost(message, repeated, tag = "2")]
        pub(super) log_records: Vec<LogRecord>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct LogRecord {
        #[prost(fixed64, tag = "1")]
        pub(super) time_unix_nano: u64,
        #[prost(fixed64, tag = "11")]
        pub(super) observed_time_unix_nano: u64,
        #[prost(message, optional, tag = "5")]
        pub(super) body: Option<AnyValue>,
        #[prost(message, repeated, tag = "6")]
        pub(super) attributes: Vec<KeyValue>,
        #[prost(string, tag = "12")]
        pub(super) event_name: String,
        #[prost(bytes, tag = "9")]
        pub(super) trace_id: Vec<u8>,
        #[prost(bytes, tag = "10")]
        pub(super) span_id: Vec<u8>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ExportTraceServiceRequest {
        #[prost(message, repeated, tag = "1")]
        pub(super) resource_spans: Vec<ResourceSpans>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ResourceSpans {
        #[prost(message, optional, tag = "1")]
        pub(super) resource: Option<Resource>,
        #[prost(message, repeated, tag = "2")]
        pub(super) scope_spans: Vec<ScopeSpans>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ScopeSpans {
        #[prost(message, optional, tag = "1")]
        pub(super) scope: Option<InstrumentationScope>,
        #[prost(message, repeated, tag = "2")]
        pub(super) spans: Vec<Span>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct Span {
        #[prost(bytes, tag = "1")]
        pub(super) trace_id: Vec<u8>,
        #[prost(bytes, tag = "2")]
        pub(super) span_id: Vec<u8>,
        #[prost(string, tag = "5")]
        pub(super) name: String,
        #[prost(fixed64, tag = "7")]
        pub(super) start_time_unix_nano: u64,
        #[prost(fixed64, tag = "8")]
        pub(super) end_time_unix_nano: u64,
        #[prost(message, repeated, tag = "9")]
        pub(super) attributes: Vec<KeyValue>,
        #[prost(message, optional, tag = "15")]
        pub(super) status: Option<Status>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct Status {
        #[prost(string, tag = "2")]
        pub(super) message: String,
        #[prost(int32, tag = "3")]
        pub(super) code: i32,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ExportMetricsServiceRequest {
        #[prost(message, repeated, tag = "1")]
        pub(super) resource_metrics: Vec<ResourceMetrics>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ResourceMetrics {
        #[prost(message, optional, tag = "1")]
        pub(super) resource: Option<Resource>,
    }

    /// `google.rpc.Status`, the body of an OTLP/HTTP error answered in protobuf.

    #[derive(Clone, PartialEq, prost::Message)]

    pub(super) struct RpcStatus {
        #[prost(int32, tag = "1")]
        pub(super) code: i32,

        #[prost(string, tag = "2")]
        pub(super) message: String,
    }

    /// `partial_success` of every Export*ServiceResponse: field 1, with the rejected count in
    /// field 1 and the message in field 2.
    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct ExportResponse {
        #[prost(message, optional, tag = "1")]
        pub(super) partial_success: Option<PartialSuccess>,
    }

    #[derive(Clone, PartialEq, prost::Message)]
    pub(super) struct PartialSuccess {
        #[prost(int64, tag = "1")]
        pub(super) rejected: i64,
        #[prost(string, tag = "2")]
        pub(super) error_message: String,
    }
}

fn from_pb(v: Option<pb::AnyValue>) -> Value {
    use pb::Kind;
    match v.and_then(|v| v.value) {
        Some(Kind::String(s)) => Value::Str(s),
        Some(Kind::Bool(b)) => Value::Bool(b),
        Some(Kind::Int(i)) => Value::Int(i),
        Some(Kind::Double(d)) => Value::Double(d),
        Some(Kind::Bytes(b)) => Value::Bytes(b),
        Some(Kind::Array(a)) => {
            Value::Array(a.values.into_iter().map(|v| from_pb(Some(v))).collect())
        }
        Some(Kind::Kvlist(l)) => Value::Map(pb_attrs(l.values).0),
        None => Value::Empty,
    }
}

fn pb_attrs(kvs: Vec<pb::KeyValue>) -> Attrs {
    Attrs(
        kvs.into_iter()
            .map(|kv| (kv.key, from_pb(kv.value)))
            .collect(),
    )
}

fn pb_resource(r: Option<pb::Resource>) -> Attrs {
    r.map(|r| pb_attrs(r.attributes)).unwrap_or_default()
}

pub fn logs_from_protobuf(body: &[u8], max: usize) -> Result<Vec<Group<LogRecord>>, DecodeError> {
    if count_protobuf(body, &[1, 2, 2])? > max {
        return Err(DecodeError::TooMany);
    }
    let req = <pb::ExportLogsServiceRequest as prost::Message>::decode(body)
        .map_err(|e| DecodeError::Protobuf(e.to_string()))?;
    let mut out = Vec::new();
    for rl in req.resource_logs {
        let resource = pb_resource(rl.resource);
        for sl in rl.scope_logs {
            out.push(Group {
                resource: resource.clone(),
                scope: sl.scope.map(|s| s.name).unwrap_or_default(),
                items: sl
                    .log_records
                    .into_iter()
                    .map(|r| LogRecord {
                        time_unix_nano: r.time_unix_nano,
                        observed_time_unix_nano: r.observed_time_unix_nano,
                        body: r.body.map(|b| from_pb(Some(b))),
                        attributes: pb_attrs(r.attributes),
                        event_name: r.event_name,
                        trace_id: r.trace_id,
                        span_id: r.span_id,
                    })
                    .collect(),
            });
        }
    }
    Ok(out)
}

pub fn spans_from_protobuf(body: &[u8], max: usize) -> Result<Vec<Group<Span>>, DecodeError> {
    if count_protobuf(body, &[1, 2, 2])? > max {
        return Err(DecodeError::TooMany);
    }
    let req = <pb::ExportTraceServiceRequest as prost::Message>::decode(body)
        .map_err(|e| DecodeError::Protobuf(e.to_string()))?;
    let mut out = Vec::new();
    for rs in req.resource_spans {
        let resource = pb_resource(rs.resource);
        for ss in rs.scope_spans {
            out.push(Group {
                resource: resource.clone(),
                scope: ss.scope.map(|s| s.name).unwrap_or_default(),
                items: ss
                    .spans
                    .into_iter()
                    .map(|s| Span {
                        trace_id: s.trace_id,
                        span_id: s.span_id,
                        name: s.name,
                        start_time_unix_nano: s.start_time_unix_nano,
                        end_time_unix_nano: s.end_time_unix_nano,
                        attributes: pb_attrs(s.attributes),
                        status_code: s.status.as_ref().map_or(0, |s| s.code),
                        status_message: s.status.map(|s| s.message).unwrap_or_default(),
                    })
                    .collect(),
            });
        }
    }
    Ok(out)
}

/// The number of resources a metrics export names; Kaeso counts no usage from metrics.
pub fn metrics_from_protobuf(body: &[u8]) -> Result<usize, DecodeError> {
    let req = <pb::ExportMetricsServiceRequest as prost::Message>::decode(body)
        .map_err(|e| DecodeError::Protobuf(e.to_string()))?;
    Ok(req.resource_metrics.len())
}

/// An OTLP/HTTP error body for a protobuf request: `google.rpc.Status` with its gRPC code.
pub fn status_protobuf(code: i32, message: &str) -> Vec<u8> {
    prost::Message::encode_to_vec(&pb::RpcStatus {
        code,
        message: message.to_owned(),
    })
}

/// How many records a protobuf export holds, counted on the wire before anything is decoded, so
/// a body of millions of tiny records is refused before it costs memory. `path` names the field
/// at each level, such as `[1, 2, 2]` for `resource_logs.scope_logs.log_records`.
pub fn count_protobuf(body: &[u8], path: &[u32]) -> Result<usize, DecodeError> {
    let bad = || DecodeError::Protobuf("the message ends inside a field".into());
    let Some((&want, deeper)) = path.split_first() else {
        return Ok(1);
    };
    let mut at = 0usize;
    let mut count = 0usize;
    while at < body.len() {
        let (key, used) = varint(body.get(at..).ok_or_else(bad)?).ok_or_else(bad)?;
        at += used;
        let (field, wire) = (key >> 3, key & 7);
        match wire {
            0 => {
                let (_, used) = varint(body.get(at..).ok_or_else(bad)?).ok_or_else(bad)?;
                at += used;
            }
            1 => at += 8,
            5 => at += 4,
            2 => {
                let (len, used) = varint(body.get(at..).ok_or_else(bad)?).ok_or_else(bad)?;
                at += used;
                let end = usize::try_from(len)
                    .ok()
                    .and_then(|len| at.checked_add(len))
                    .ok_or_else(bad)?;
                let inner = body.get(at..end).ok_or_else(bad)?;
                if u64::from(want) == field {
                    count = count.saturating_add(count_protobuf(inner, deeper)?);
                }
                at = end;
            }
            _ => {
                return Err(DecodeError::Protobuf(
                    "a field of an unknown wire type".into(),
                ));
            }
        }
    }
    if at > body.len() {
        return Err(bad());
    }
    Ok(count)
}

fn varint(b: &[u8]) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for (i, byte) in b.iter().take(10).enumerate() {
        v |= u64::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

/// How many records a JSON export holds, read from the parsed body before its records are.
pub fn count_json(root: &Json, outer: &str, inner: &str, items: &str) -> usize {
    root.get(outer)
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .flat_map(|r| r.get(inner).and_then(Json::as_array).into_iter().flatten())
        .map(|s| s.get(items).and_then(Json::as_array).map_or(0, Vec::len))
        .sum()
}

/// The body of an export's answer: empty when everything was taken, else `partial_success`
/// with what was refused and why.
pub fn response_protobuf(rejected: i64, message: &str) -> Vec<u8> {
    let r = pb::ExportResponse {
        partial_success: (rejected > 0 || !message.is_empty()).then(|| pb::PartialSuccess {
            rejected,
            error_message: message.to_owned(),
        }),
    };
    prost::Message::encode_to_vec(&r)
}

// JSON: the OTLP/JSON mapping, read leniently where clients differ (numbers as strings).

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JKeyValue {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: Option<Json>,
}

fn json_u64(v: &Json) -> u64 {
    match v {
        Json::Number(n) => n.as_u64().unwrap_or(0),
        Json::String(s) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn from_json(v: &Json) -> Value {
    let Some(map) = v.as_object() else {
        return Value::Empty;
    };
    if let Some(s) = map.get("stringValue").and_then(Json::as_str) {
        return Value::Str(s.to_owned());
    }
    if let Some(b) = map.get("boolValue").and_then(Json::as_bool) {
        return Value::Bool(b);
    }
    if let Some(i) = map.get("intValue") {
        return match i {
            Json::Number(n) => n.as_i64().map_or(Value::Empty, Value::Int),
            Json::String(s) => s.parse().map_or(Value::Empty, Value::Int),
            _ => Value::Empty,
        };
    }
    if let Some(d) = map.get("doubleValue") {
        return match d {
            Json::Number(n) => n.as_f64().map_or(Value::Empty, Value::Double),
            Json::String(s) => s.parse().map_or(Value::Empty, Value::Double),
            _ => Value::Empty,
        };
    }
    if let Some(a) = map.get("arrayValue") {
        return Value::Array(
            a.get("values")
                .and_then(Json::as_array)
                .map(|vs| vs.iter().map(from_json).collect())
                .unwrap_or_default(),
        );
    }
    if let Some(l) = map.get("kvlistValue") {
        return Value::Map(json_attrs(l.get("values")).0);
    }
    if let Some(b) = map.get("bytesValue").and_then(Json::as_str) {
        use base64::Engine;
        return base64::engine::general_purpose::STANDARD
            .decode(b)
            .map_or(Value::Empty, Value::Bytes);
    }
    Value::Empty
}

fn json_attrs(v: Option<&Json>) -> Attrs {
    let Some(items) = v.and_then(Json::as_array) else {
        return Attrs::default();
    };
    Attrs(
        items
            .iter()
            .filter_map(|kv| serde_json::from_value::<JKeyValue>(kv.clone()).ok())
            .map(|kv| (kv.key, kv.value.as_ref().map_or(Value::Empty, from_json)))
            .collect(),
    )
}

fn hex(v: Option<&Json>) -> Vec<u8> {
    let Some(s) = v.and_then(Json::as_str) else {
        return Vec::new();
    };
    (0..s.len() / 2)
        .filter_map(|i| s.get(i * 2..i * 2 + 2))
        .filter_map(|b| u8::from_str_radix(b, 16).ok())
        .collect()
}

fn groups<T>(
    root: &Json,
    outer: &str,
    inner: &str,
    items: &str,
    each: impl Fn(&Json) -> T,
) -> Result<Vec<Group<T>>, DecodeError> {
    let list = root
        .get(outer)
        .ok_or_else(|| DecodeError::Json(format!("`{outer}` is missing")))?
        .as_array()
        .ok_or_else(|| DecodeError::Json(format!("`{outer}` is not a list")))?;
    let mut out = Vec::new();
    for r in list {
        let resource = json_attrs(r.get("resource").and_then(|r| r.get("attributes")));
        for s in r.get(inner).and_then(Json::as_array).into_iter().flatten() {
            out.push(Group {
                resource: resource.clone(),
                scope: s
                    .get("scope")
                    .and_then(|s| s.get("name"))
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                items: s
                    .get(items)
                    .and_then(Json::as_array)
                    .into_iter()
                    .flatten()
                    .map(&each)
                    .collect(),
            });
        }
    }
    Ok(out)
}

fn parse(body: &[u8]) -> Result<Json, DecodeError> {
    serde_json::from_slice(body).map_err(|e| DecodeError::Json(e.to_string()))
}

pub fn logs_from_json(body: &[u8], max: usize) -> Result<Vec<Group<LogRecord>>, DecodeError> {
    let root = parse(body)?;
    if count_json(&root, "resourceLogs", "scopeLogs", "logRecords") > max {
        return Err(DecodeError::TooMany);
    }
    groups(&root, "resourceLogs", "scopeLogs", "logRecords", |r| {
        LogRecord {
            time_unix_nano: r.get("timeUnixNano").map_or(0, json_u64),
            observed_time_unix_nano: r.get("observedTimeUnixNano").map_or(0, json_u64),
            body: r.get("body").map(from_json),
            attributes: json_attrs(r.get("attributes")),
            event_name: r
                .get("eventName")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_owned(),
            trace_id: hex(r.get("traceId")),
            span_id: hex(r.get("spanId")),
        }
    })
}

pub fn spans_from_json(body: &[u8], max: usize) -> Result<Vec<Group<Span>>, DecodeError> {
    let root = parse(body)?;
    if count_json(&root, "resourceSpans", "scopeSpans", "spans") > max {
        return Err(DecodeError::TooMany);
    }
    groups(&root, "resourceSpans", "scopeSpans", "spans", |s| Span {
        trace_id: hex(s.get("traceId")),
        span_id: hex(s.get("spanId")),
        name: s
            .get("name")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned(),
        start_time_unix_nano: s.get("startTimeUnixNano").map_or(0, json_u64),
        end_time_unix_nano: s.get("endTimeUnixNano").map_or(0, json_u64),
        attributes: json_attrs(s.get("attributes")),
        status_code: s
            .get("status")
            .and_then(|st| st.get("code"))
            .and_then(Json::as_i64)
            .and_then(|c| i32::try_from(c).ok())
            .unwrap_or(0),
        status_message: s
            .get("status")
            .and_then(|st| st.get("message"))
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

pub fn metrics_from_json(body: &[u8]) -> Result<usize, DecodeError> {
    let root = parse(body)?;
    root.get("resourceMetrics")
        .and_then(Json::as_array)
        .map(Vec::len)
        .ok_or_else(|| DecodeError::Json("`resourceMetrics` is missing".into()))
}

/// The JSON answer to an export of `signal` (`logs`, `traces` or `metrics`), which names its
/// rejected count after what it holds.
pub fn response_json(signal: &str, rejected: i64, message: &str) -> Json {
    if rejected == 0 && message.is_empty() {
        return serde_json::json!({});
    }
    let field = match signal {
        "traces" => "rejectedSpans",
        "metrics" => "rejectedDataPoints",
        _ => "rejectedLogRecords",
    };
    serde_json::json!({
        "partialSuccess": { field: rejected.to_string(), "errorMessage": message }
    })
}
