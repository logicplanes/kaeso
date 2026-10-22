use std::borrow::Cow;
use std::fmt;

use kaeso_kpp::{DialectId, KppError};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;

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
    /// Bytes of the body without the whitespace between the tokens of the request
    /// object, its messages, items or inputs, their content blocks and the list of
    /// tools: the most its decoded values can hold. Whitespace inside any other value
    /// counts, since a value kept raw, such as a tool's parameters, keeps it.
    pub bytes: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Node {
    Top(DialectId),
    Items,
    Item,
    Content,
    Block,
    Tools,
    Inputs,
    Input,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Over {
    Items,
    Blocks,
    Tools,
    Inputs,
}

#[derive(Default)]
struct Tally {
    shape: RequestShape,
    in_message: usize,
    over: Option<Over>,
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
    use DialectId::{AnthropicMessages, OpenAiChat, OpenAiEmbeddings, OpenAiResponses};
    Some(match (node, key) {
        (Node::Top(OpenAiResponses), "input") => Node::Items,
        (Node::Top(OpenAiEmbeddings), "input") => Node::Inputs,
        (Node::Top(AnthropicMessages | OpenAiChat), "messages") => Node::Items,
        (Node::Top(AnthropicMessages), "system") => Node::Content,
        (Node::Top(OpenAiChat), "functions") => Node::Tools,
        (Node::Top(AnthropicMessages | OpenAiChat | OpenAiResponses), "tools") => Node::Tools,
        (Node::Item | Node::Block | Node::Content, "content" | "output") => Node::Content,
        _ => return None,
    })
}

struct Walk<'t> {
    node: Node,
    tally: &'t mut Tally,
}

impl Walk<'_> {
    fn at(&mut self, node: Node) -> Walk<'_> {
        Walk {
            node,
            tally: &mut *self.tally,
        }
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
        Ok(())
    }

    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<(), E> {
        match self.node {
            Node::Items => self.tally.count(Over::Items),
            Node::Content => self.tally.count(Over::Blocks),
            Node::Inputs | Node::Input => self.tally.count(Over::Inputs),
            _ => Ok(()),
        }
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut seq: A) -> Result<(), A::Error> {
        let (counted, element) = match self.node {
            Node::Items => (Some(Over::Items), Some(Node::Item)),
            Node::Content => (Some(Over::Blocks), Some(Node::Block)),
            Node::Tools => (Some(Over::Tools), None),
            Node::Inputs => (None, Some(Node::Input)),
            Node::Input => {
                self.tally.count(Over::Inputs)?;
                (None, None)
            }
            Node::Top(_) | Node::Item | Node::Block => (None, None),
        };
        loop {
            if self.node == Node::Items {
                self.tally.in_message = 0;
            }
            let more = match element {
                Some(node) => seq.next_element_seed(self.at(node))?.is_some(),
                None => seq.next_element_seed(Opaque(&mut *self.tally))?.is_some(),
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
        if self.node == Node::Content {
            self.tally.count(Over::Blocks)?;
        }
        while let Some(Key(key)) = map.next_key::<Key<'de>>()? {
            match child(self.node, &key) {
                Some(node) => {
                    if matches!(self.node, Node::Top(_)) {
                        self.tally.in_message = 0;
                    }
                    map.next_value_seed(self.at(node))?;
                }
                None => map.next_value_seed(Opaque(&mut *self.tally))?,
            }
        }
        Ok(())
    }
}

struct Opaque<'t>(&'t mut Tally);

impl<'de> DeserializeSeed<'de> for Opaque<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        let raw = <&'de RawValue as serde::Deserialize<'de>>::deserialize(d)?;
        self.0.shape.bytes += whitespace_outside_strings(raw.get().as_bytes());
        Ok(())
    }
}

fn whitespace_outside_strings(json: &[u8]) -> usize {
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
    let (message, param) = match over {
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

/// Counts the messages, content blocks and tools of a request body without decoding it,
/// and refuses one over [`MAX_MESSAGES`], [`MAX_BLOCKS_PER_MESSAGE`], [`MAX_TOOLS`] or
/// [`MAX_EMBEDDING_INPUTS`] with an `InvalidRequest` error that names the field, and one
/// that nests deeper than [`MAX_DEPTH`] with an `InvalidRequest` error.
///
/// Every codec's `decode_request` runs this first. A body that is not JSON, or not shaped
/// like the dialect's request, is counted as far as it can be read, and decoding it then
/// reports what is wrong.
pub fn request_shape(dialect: DialectId, body: &[u8]) -> Result<RequestShape, KppError> {
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
    }
    .deserialize(&mut de);
    match (walked, tally.over) {
        (Err(_), Some(over)) => Err(refusal(dialect, over)),
        _ => Ok(tally.shape),
    }
}

// serde_json skips ignored and raw values without its recursion limit, so the codecs'
// walks over them would follow any depth; this scan keeps no stack to overflow.
fn significant_bytes(body: &[u8], depth_limit: usize) -> Option<usize> {
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
