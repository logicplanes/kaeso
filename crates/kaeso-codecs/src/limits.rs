use std::borrow::Cow;
use std::fmt;

use kaeso_kpp::{DialectId, KppError};
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;

use crate::openai_chat::PartContext;

/// The most messages (Anthropic Messages, OpenAI Chat Completions) or input items
/// (OpenAI Responses) one request may hold: Anthropic's documented limit per request.
pub const MAX_MESSAGES: usize = 100_000;

/// The most content blocks one message, input item or system prompt may hold, counting
/// the blocks nested in its tool results.
pub const MAX_BLOCKS_PER_MESSAGE: usize = 10_000;

/// The most tools one request may define: the catalog size Anthropic's tool search takes.
pub const MAX_TOOLS: usize = 10_000;

/// The most inputs one OpenAI Embeddings request may hold: OpenAI's documented limit.
pub const MAX_EMBEDDING_INPUTS: usize = 2_048;

/// The deepest one request body may nest objects and arrays, the outermost object
/// counted: the most serde_json parses whole under its recursion limit of 128.
pub const MAX_DEPTH: usize = 127;

/// What a request body holds, counted without decoding it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RequestShape {
    /// Messages, input items or embedding inputs.
    pub items: usize,
    /// Content blocks in all of them and in the system prompt.
    pub blocks: usize,
    /// Tool definitions.
    pub tools: usize,
    /// Token ids in the inputs of an embeddings request.
    pub tokens: usize,
    /// Bytes of the body without the whitespace that decoding drops: the whitespace
    /// between the tokens of the request object, its messages, items or inputs, their
    /// content blocks and the list of tools. Whitespace inside any other value counts,
    /// and so does all whitespace inside a message, item or block that its decoder keeps
    /// raw, such as a content block that isn't an object or is of a type it doesn't
    /// know, or any input item of an OpenAI Responses request, since a raw value keeps
    /// it byte for byte.
    pub bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Chat(PartContext),
    Anthropic(u8),
}

pub(crate) enum Form {
    Raw,
    Typed,
    Nests(Scope),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Node {
    Top(DialectId),
    Items(DialectId),
    Item(DialectId),
    Content(Option<Scope>),
    Block(Option<Scope>),
    Tools,
    ToolSets,
    ToolSet,
    Inputs,
    Input,
    EmbedRequests,
    EmbedRequest,
    Token,
}

impl Node {
    // The form when it doesn't depend on the value, so the walk needs no copy of it to look at.
    fn fixed_form(self) -> Option<Form> {
        match self {
            Node::Item(DialectId::AnthropicMessages) => Some(Form::Nests(Scope::Anthropic(0))),
            _ => None,
        }
    }

    // The form `form` gives, from a leading `type`; a later repeated `type` makes the walk
    // start over the way `form` reads it.
    fn leading_form(self, raw: &RawValue) -> Option<Form> {
        let Node::Block(Some(Scope::Anthropic(depth))) = self else {
            return None;
        };
        crate::anthropic_messages::block_form_of(crate::wire::leading_type(raw)?, depth)
    }

    fn form(self, raw: &RawValue) -> Form {
        match self {
            Node::Item(DialectId::OpenAiChat) => crate::openai_chat::message_form(raw),
            Node::Item(DialectId::AnthropicMessages) => Form::Nests(Scope::Anthropic(0)),
            Node::Block(Some(Scope::Chat(parts))) => crate::openai_chat::part_form(raw, parts),
            Node::Block(Some(Scope::Anthropic(depth))) => {
                crate::anthropic_messages::block_form(raw, depth)
            }
            // The Responses decoder keeps any item or part raw that doesn't parse as its
            // type, which only decoding it can tell.
            _ => Form::Raw,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Over {
    Items,
    Blocks,
    Tools,
    Inputs,
}

#[derive(Clone, Default)]
struct Tally {
    shape: RequestShape,
    in_message: usize,
    over: Option<Over>,
    second_type: bool,
}

impl Tally {
    fn count<E: serde::de::Error>(&mut self, what: Over) -> Result<(), E> {
        let s = &mut self.shape;
        let over = match what {
            Over::Items => {
                s.items += 1;
                s.items > MAX_MESSAGES
            }
            Over::Inputs => {
                s.items += 1;
                s.items > MAX_EMBEDDING_INPUTS
            }
            Over::Tools => {
                s.tools += 1;
                s.tools > MAX_TOOLS
            }
            Over::Blocks => {
                s.blocks += 1;
                self.in_message += 1;
                self.in_message > MAX_BLOCKS_PER_MESSAGE
            }
        };
        if over {
            self.over = Some(what);
            return Err(E::custom("over a request limit"));
        }
        Ok(())
    }
}

struct Key<'de>(Cow<'de, str>);

impl<'de> serde::Deserialize<'de> for Key<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Key<'de>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a key")
            }
            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E> {
                Ok(Key(Cow::Borrowed(v)))
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Key(Cow::Owned(v.to_owned())))
            }
        }
        d.deserialize_str(V)
    }
}

fn child(node: Node, key: &str) -> Option<Node> {
    use DialectId::{
        AnthropicMessages, Gemini, GeminiEmbeddings, OpenAiChat, OpenAiEmbeddings, OpenAiResponses,
    };
    Some(match (node, key) {
        (Node::Top(d @ Gemini), "contents") => Node::Items(d),
        (Node::Top(d @ Gemini), "systemInstruction") => Node::Item(d),
        (Node::Top(d @ Gemini), "generateContentRequest") => Node::Top(d),
        (Node::Top(Gemini), "tools") => Node::ToolSets,
        (Node::ToolSet, "functionDeclarations") => Node::Tools,
        (Node::Item(Gemini | GeminiEmbeddings), "parts") => Node::Content(None),
        (Node::Top(GeminiEmbeddings), "requests") => Node::EmbedRequests,
        (Node::Top(GeminiEmbeddings) | Node::EmbedRequest, "content") => {
            Node::Item(GeminiEmbeddings)
        }
        (Node::Top(d @ OpenAiResponses), "input") => Node::Items(d),
        (Node::Top(OpenAiEmbeddings), "input") => Node::Inputs,
        (Node::Top(d @ (AnthropicMessages | OpenAiChat)), "messages") => Node::Items(d),
        (Node::Top(AnthropicMessages), "system") => Node::Content(Some(Scope::Anthropic(0))),
        (Node::Top(OpenAiChat), "functions") => Node::Tools,
        (Node::Top(AnthropicMessages | OpenAiChat | OpenAiResponses), "tools") => Node::Tools,
        (Node::Item(_) | Node::Block(_) | Node::Content(_), "content" | "output") => {
            Node::Content(None)
        }
        (Node::Item(OpenAiChat), "tool_calls") => Node::Content(None),
        _ => return None,
    })
}

struct Walk<'t> {
    node: Node,
    tally: &'t mut Tally,
    kept: bool,
    nests: Option<Scope>,
    one_type: bool,
}

impl Walk<'_> {
    fn at(&mut self, node: Node) -> Walk<'_> {
        Walk {
            node,
            tally: &mut *self.tally,
            kept: self.kept,
            nests: None,
            one_type: false,
        }
    }

    fn opaque(&mut self) -> Opaque<'_> {
        Opaque {
            tally: &mut *self.tally,
            kept: self.kept,
        }
    }

    fn token(self) {
        if matches!(self.node, Node::Input | Node::Token) {
            self.tally.shape.tokens += 1;
        }
    }

    fn walk_raw<E: serde::de::Error>(self, raw: &RawValue) -> Result<(), E> {
        let mut de = serde_json::Deserializer::from_str(raw.get());
        self.deserialize(&mut de).map_err(E::custom)
    }
}

impl<'de> DeserializeSeed<'de> for Walk<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Walk<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON")
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        self.token();
        Ok(())
    }

    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        self.token();
        Ok(())
    }

    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        self.token();
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<(), E> {
        match self.node {
            Node::Items(_) => self.tally.count(Over::Items),
            Node::Content(_) => self.tally.count(Over::Blocks),
            Node::Inputs | Node::Input => self.tally.count(Over::Inputs),
            _ => Ok(()),
        }
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut seq: A) -> Result<(), A::Error> {
        let (counted, element) = match self.node {
            Node::Items(d) => (Some(Over::Items), Some(Node::Item(d))),
            Node::Content(scope) => (Some(Over::Blocks), Some(Node::Block(scope))),
            Node::Tools => (Some(Over::Tools), None),
            Node::Inputs => (None, Some(Node::Input)),
            Node::ToolSets => (None, Some(Node::ToolSet)),
            Node::EmbedRequests => (Some(Over::Inputs), Some(Node::EmbedRequest)),
            Node::Input => {
                self.tally.count(Over::Inputs)?;
                (None, Some(Node::Token))
            }
            Node::Top(_)
            | Node::Item(_)
            | Node::Block(_)
            | Node::Token
            | Node::ToolSet
            | Node::EmbedRequest => (None, None),
        };
        loop {
            if matches!(self.node, Node::Items(_)) {
                self.tally.in_message = 0;
            }
            let more = match element {
                Some(node @ (Node::Item(_) | Node::Block(_))) if !self.kept => seq
                    .next_element_seed(Element {
                        node,
                        tally: &mut *self.tally,
                    })?
                    .is_some(),
                Some(node) => seq.next_element_seed(self.at(node))?.is_some(),
                None => seq.next_element_seed(self.opaque())?.is_some(),
            };
            if !more {
                return Ok(());
            }
            if let Some(what) = counted {
                self.tally.count(what)?;
            }
        }
    }

    fn visit_map<A: MapAccess<'de>>(mut self, mut map: A) -> Result<(), A::Error> {
        if matches!(self.node, Node::Content(_)) {
            self.tally.count(Over::Blocks)?;
        }
        let typed_object = !self.kept && matches!(self.node, Node::Item(_) | Node::Block(_));
        let mut types = 0usize;
        while let Some(Key(key)) = map.next_key::<Key<'de>>()? {
            if self.one_type && key == "type" {
                types += 1;
                if types > 1 {
                    self.tally.second_type = true;
                    return Err(serde::de::Error::custom("a repeated type"));
                }
            }
            if self.node == Node::ToolSet && key != "functionDeclarations" {
                self.tally.count(Over::Tools)?;
            }
            if self.node == Node::Top(DialectId::GeminiEmbeddings) && key == "content" {
                self.tally.count(Over::Inputs)?;
            }
            match child(self.node, &key) {
                Some(_) if typed_object && key == "content" && self.nests.is_some() => {
                    let node = Node::Content(self.nests);
                    map.next_value_seed(self.at(node))?;
                }
                Some(node) if typed_object => map.next_value_seed(Kept {
                    node,
                    tally: &mut *self.tally,
                })?,
                Some(node) => {
                    if matches!(self.node, Node::Top(_)) {
                        self.tally.in_message = 0;
                    }
                    map.next_value_seed(self.at(node))?;
                }
                None => map.next_value_seed(self.opaque())?,
            }
        }
        Ok(())
    }
}

struct Element<'t> {
    node: Node,
    tally: &'t mut Tally,
}

impl<'de> DeserializeSeed<'de> for Element<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        if let Some(Form::Nests(scope)) = self.node.fixed_form() {
            return Walk {
                node: self.node,
                tally: self.tally,
                kept: false,
                nests: Some(scope),
                one_type: false,
            }
            .deserialize(d);
        }
        let raw = <&'de RawValue as serde::Deserialize<'de>>::deserialize(d)?;
        if let Some(form @ (Form::Typed | Form::Nests(_))) = self.node.leading_form(raw) {
            let before = self.tally.clone();
            let walked = Walk {
                node: self.node,
                tally: &mut *self.tally,
                kept: false,
                nests: match form {
                    Form::Nests(scope) => Some(scope),
                    _ => None,
                },
                one_type: true,
            }
            .walk_raw(raw);
            if !self.tally.second_type {
                return walked;
            }
            *self.tally = before;
        }
        let (kept, nests) = match self.node.form(raw) {
            Form::Raw => {
                self.tally.shape.bytes += whitespace_outside_strings(raw.get().as_bytes());
                (true, None)
            }
            Form::Typed => (false, None),
            Form::Nests(scope) => (false, Some(scope)),
        };
        Walk {
            node: self.node,
            tally: self.tally,
            kept,
            nests,
            one_type: false,
        }
        .walk_raw(raw)
    }
}

struct Kept<'t> {
    node: Node,
    tally: &'t mut Tally,
}

impl<'de> DeserializeSeed<'de> for Kept<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        let raw = <&'de RawValue as serde::Deserialize<'de>>::deserialize(d)?;
        self.tally.shape.bytes += whitespace_outside_strings(raw.get().as_bytes());
        Walk {
            node: self.node,
            tally: self.tally,
            kept: true,
            nests: None,
            one_type: false,
        }
        .walk_raw(raw)
    }
}

struct Opaque<'t> {
    tally: &'t mut Tally,
    kept: bool,
}

impl<'de> DeserializeSeed<'de> for Opaque<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        if self.kept {
            return <IgnoredAny as serde::Deserialize<'de>>::deserialize(d).map(|_| ());
        }
        let raw = <&'de RawValue as serde::Deserialize<'de>>::deserialize(d)?;
        self.tally.shape.bytes += whitespace_outside_strings(raw.get().as_bytes());
        Ok(())
    }
}

fn whitespace_outside_strings(json: &[u8]) -> usize {
    // A raw value starts at its first byte: only an object or an array has whitespace outside
    // strings, and a string, the bulk of most requests, needs no scan.
    if !matches!(json.first(), Some(b'{' | b'[')) {
        return 0;
    }
    let mut whitespace = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in json {
        match byte {
            _ if escaped => escaped = false,
            b'\\' if in_string => escaped = true,
            b'"' => in_string = !in_string,
            _ if !in_string && byte.is_ascii_whitespace() => whitespace += 1,
            _ => {}
        }
    }
    whitespace
}

fn refusal(dialect: DialectId, over: Over) -> KppError {
    let responses = dialect == DialectId::OpenAiResponses;
    let gemini = matches!(dialect, DialectId::Gemini | DialectId::GeminiEmbeddings);
    let (message, param) = match over {
        Over::Items if gemini => (
            format!(
                "`contents` holds more than {MAX_MESSAGES} contents; Kaeso takes at most {MAX_MESSAGES}."
            ),
            "contents",
        ),
        Over::Blocks if gemini => (
            format!(
                "A content holds more than {MAX_BLOCKS_PER_MESSAGE} parts; Kaeso takes at most {MAX_BLOCKS_PER_MESSAGE}."
            ),
            "contents",
        ),
        Over::Inputs if gemini => (
            format!(
                "`requests` holds more than {MAX_EMBEDDING_INPUTS} requests; Kaeso takes at most {MAX_EMBEDDING_INPUTS}."
            ),
            "requests",
        ),
        Over::Items if responses => (
            format!(
                "`input` holds more than {MAX_MESSAGES} items; Kaeso takes at most {MAX_MESSAGES}."
            ),
            "input",
        ),
        Over::Items => (
            format!(
                "`messages` holds more than {MAX_MESSAGES} messages; Kaeso takes at most {MAX_MESSAGES}."
            ),
            "messages",
        ),
        Over::Blocks => (
            format!(
                "A message holds more than {MAX_BLOCKS_PER_MESSAGE} content blocks; Kaeso takes at most {MAX_BLOCKS_PER_MESSAGE}."
            ),
            if responses { "input" } else { "messages" },
        ),
        Over::Tools => (
            format!("`tools` holds more than {MAX_TOOLS} tools; Kaeso takes at most {MAX_TOOLS}."),
            "tools",
        ),
        Over::Inputs => (
            format!(
                "`input` holds more than {MAX_EMBEDDING_INPUTS} inputs; Kaeso takes at most {MAX_EMBEDDING_INPUTS}."
            ),
            "input",
        ),
    };
    KppError::invalid_request(message)
        .with_param(param)
        .with_status(400)
}

/// A request body that passed [`request_shape`]'s checks for one dialect, kept with its
/// shape. [`crate::dispatch::decode_checked_request`] decodes it without counting it again,
/// so a caller that needs the shape before decoding, to budget memory, scans the body once.
///
/// ```
/// use kaeso_codecs::{dispatch, limits::Checked, HeaderList};
/// use kaeso_kpp::{DialectId, RequestKind};
///
/// let body = br#"{"model":"gpt-5.5","messages":[{"role":"user","content":"Hi"}]}"#;
/// let checked = Checked::new(DialectId::OpenAiChat, &body[..]).unwrap();
/// assert_eq!(checked.shape().items, 1);
/// let req = dispatch::decode_checked_request(RequestKind::Generate, &checked, &HeaderList::new());
/// assert!(req.is_ok());
/// ```
#[derive(Clone, Debug)]
pub struct Checked<B> {
    dialect: DialectId,
    body: B,
    shape: RequestShape,
}

impl<B: AsRef<[u8]>> Checked<B> {
    /// Checks `body` as a request in `dialect`, as [`request_shape`] does.
    pub fn new(dialect: DialectId, body: B) -> Result<Checked<B>, KppError> {
        let shape = request_shape(dialect, body.as_ref())?;
        Ok(Checked {
            dialect,
            body,
            shape,
        })
    }

    /// The dialect the body was checked as.
    pub fn dialect(&self) -> DialectId {
        self.dialect
    }

    /// What the body holds.
    pub fn shape(&self) -> &RequestShape {
        &self.shape
    }

    /// The body.
    pub fn body(&self) -> &B {
        &self.body
    }

    /// The body, giving up the check.
    pub fn into_body(self) -> B {
        self.body
    }
}

/// Counts the messages, content blocks and tools of a request body without decoding it,
/// and refuses one over [`MAX_MESSAGES`], [`MAX_BLOCKS_PER_MESSAGE`], [`MAX_TOOLS`] or
/// [`MAX_EMBEDDING_INPUTS`] with an `InvalidRequest` error that names the field, and one
/// that nests deeper than [`MAX_DEPTH`] with an `InvalidRequest` error.
///
/// Every codec's `decode_request` runs this first. A body that is not JSON, or not shaped
/// like the dialect's request, is counted as far as it can be read, and decoding it then
/// reports what is wrong.
pub fn request_shape(dialect: DialectId, body: &[u8]) -> Result<RequestShape, KppError> {
    // An image edit may be a `multipart/form-data` upload, whose bytes are no JSON to walk;
    // its parts are counted and bounded by the multipart reader instead.
    if dialect == DialectId::OpenAiImages && !looks_like_json(body) {
        return Ok(RequestShape {
            bytes: body.len(),
            ..RequestShape::default()
        });
    }
    let Some(bytes) = significant_bytes(body, MAX_DEPTH) else {
        return Err(KppError::invalid_request(format!(
            "The body nests more than {MAX_DEPTH} levels deep; Kaeso takes at most {MAX_DEPTH}."
        ))
        .with_status(400));
    };
    let mut tally = Tally::default();
    tally.shape.bytes = bytes;
    let mut de = serde_json::Deserializer::from_slice(body);
    let walked = Walk {
        node: Node::Top(dialect),
        tally: &mut tally,
        kept: false,
        nests: None,
        one_type: false,
    }
    .deserialize(&mut de);
    match (walked, tally.over) {
        (Err(_), Some(over)) => Err(refusal(dialect, over)),
        _ => Ok(tally.shape),
    }
}

fn looks_like_json(body: &[u8]) -> bool {
    body.iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| *b == b'{')
}

// serde_json skips ignored and raw values without its recursion limit, so the codecs'
// walks over them would follow any depth; this scan keeps no stack to overflow.
fn significant_bytes(body: &[u8], depth_limit: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut bytes = 0usize;
    let mut i = 0usize;
    while let Some(&byte) = body.get(i) {
        if byte == b'"' {
            let end = string_end(body, i + 1);
            bytes += end - i;
            i = end;
            continue;
        }
        if !byte.is_ascii_whitespace() {
            bytes += 1;
        }
        match byte {
            b'{' | b'[' => {
                depth += 1;
                if depth > depth_limit {
                    return None;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    Some(bytes)
}

// Just past the quote that closes a string whose content starts at `from`, or the end of the
// body for one that never closes; every byte in between counts, an escaped quote included.
fn string_end(body: &[u8], mut from: usize) -> usize {
    loop {
        let rest = body.get(from..).unwrap_or_default();
        from += quote_or_backslash(rest);
        match body.get(from) {
            Some(b'"') => return from + 1,
            Some(_) => from += 2,
            None => return body.len(),
        }
        if from >= body.len() {
            return body.len();
        }
    }
}

// The index of the first `"` or `\` in `bytes`, or its length, eight bytes at a time: strings,
// most of a request's bytes, hold neither for long stretches.
fn quote_or_backslash(bytes: &[u8]) -> usize {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGH: u64 = 0x8080_8080_8080_8080;
    let quotes = ONES * u64::from(b'"');
    let backslashes = ONES * u64::from(b'\\');
    let (words, rest) = bytes.as_chunks::<8>();
    for (i, eight) in words.iter().enumerate() {
        let word = u64::from_le_bytes(*eight);
        let (q, b) = (word ^ quotes, word ^ backslashes);
        let found = ((q.wrapping_sub(ONES) & !q) | (b.wrapping_sub(ONES) & !b)) & HIGH;
        if found != 0 {
            return i * 8 + (found.trailing_zeros() / 8) as usize;
        }
    }
    words.len() * 8
        + rest
            .iter()
            .position(|&c| c == b'"' || c == b'\\')
            .unwrap_or(rest.len())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn byte_by_byte(body: &[u8], depth_limit: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut bytes = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for &byte in body {
            if in_string || !byte.is_ascii_whitespace() {
                bytes += 1;
            }
            if in_string {
                match byte {
                    _ if escaped => escaped = false,
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > depth_limit {
                        return None;
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        Some(bytes)
    }

    fn jsonish() -> impl Strategy<Value = Vec<u8>> {
        let byte = prop_oneof![
            Just(b'"'),
            Just(b'\\'),
            Just(b'{'),
            Just(b'}'),
            Just(b'['),
            Just(b']'),
            Just(b' '),
            Just(b'\n'),
            Just(b'a'),
            any::<u8>(),
        ];
        proptest::collection::vec(byte, 0..200)
    }

    proptest! {
        #[test]
        fn significant_bytes_count_as_a_byte_by_byte_scan_does(body in jsonish(), limit in 0usize..6) {
            prop_assert_eq!(significant_bytes(&body, limit), byte_by_byte(&body, limit));
        }
    }

    #[test]
    fn strings_longer_than_a_word_and_escapes_at_every_offset_count_alike() {
        for pad in 0..20 {
            let filler = "x".repeat(pad);
            for body in [
                format!(r#"{{"a": "{filler}\"{filler}", "b" : [ 1 ] }}"#),
                format!(r#"["{filler}\\", "{filler}"]"#),
                format!(r#""{filler}\"#),
                format!(r#""{filler}"#),
            ] {
                assert_eq!(
                    significant_bytes(body.as_bytes(), 3),
                    byte_by_byte(body.as_bytes(), 3),
                    "{body}"
                );
            }
        }
    }
}
