use base64::Engine as _;
use kaeso_kpp::{
    Block, DocumentBlock, Extras, ImageBlock, KppError, Opaque, OpaqueKind, Origin, RawItem,
    RawJson, ReasoningBlock, Source, TextBlock, ToolCallBlock, ToolResultBlock,
};
use serde_json::Value;

use super::D;
use super::wire::{BlobW, FileDataW, FunctionCallW, FunctionResponseW, PartW, Raw, ResponsePartW};
use crate::cross::reasoning_replayable;
use crate::wire::{
    Ext, add_nested_extra, extra_for, extras_from, nested_extra_for, verbatim_from_form,
    verbatim_into_form,
};

/// Ids Kaeso gives function calls that Gemini sent without one; they are left out again
/// when the conversation goes back to Gemini.
pub(crate) const GENERATED_ID_PREFIX: &str = "kaeso_fc_";

/// The documented signature that tells Gemini to skip its check of a function call it
/// didn't write (thought signatures FAQ, "dummy signatures").
pub(crate) const DUMMY_SIGNATURE: &str = "skip_thought_signature_validator";

pub(crate) const FORM_NO_ID: &str = "no_id";
pub(crate) const FORM_NO_ARGS: &str = "no_args";
pub(crate) const FORM_NAME: &str = "name";
pub(crate) const FORM_RESPONSE: &str = "response";
pub(crate) const FORM_THOUGHT: &str = "thought";
pub(crate) const FORM_MIME: &str = "mime_type";
const FORM_THOUGHT_PART: &str = "thought_part";

const INLINE: &str = "inlineData";
const FILE: &str = "fileData";
const CALL: &str = "functionCall";
const RESPONSE: &str = "functionResponse";
const FILES_URL: &str = "https://generativelanguage.googleapis.com/v1beta/files/";

pub(crate) fn signature(origin: &Origin, data: &str) -> Opaque {
    Opaque::new(origin.clone(), OpaqueKind::GeminiThoughtSignature, data)
}

pub(crate) fn carrier(origin: &Origin, data: &str) -> ReasoningBlock {
    let mut block = ReasoningBlock::new(origin.clone());
    block.opaque.push(signature(origin, data));
    block
}

/// The signature a Gemini reasoning block only carries for the part after it: a block with
/// no text and one thought signature, which a `thought: true` part never decodes to.
pub(crate) fn carried_signature<'a>(block: &'a ReasoningBlock, target: &Origin) -> Option<&'a str> {
    if !block.text.is_empty()
        || !block.summary.is_empty()
        || block.extras.form_flag(D, FORM_THOUGHT_PART)
        || !reasoning_replayable(block, target)
    {
        return None;
    }
    match block.opaque.as_slice() {
        [o] if o.kind == OpaqueKind::GeminiThoughtSignature => Some(&o.data),
        _ => None,
    }
}

fn thought_signature(block: &ReasoningBlock) -> Option<&str> {
    block
        .opaque_of(OpaqueKind::GeminiThoughtSignature)
        .map(|o| o.data.as_str())
}

fn is_media_without_block(mime: &str) -> bool {
    let lower = mime.to_ascii_lowercase();
    lower.starts_with("audio/") || lower.starts_with("video/")
}

fn source_block(source: Source, image: bool, extras: Extras) -> Block {
    if image {
        let mut b = ImageBlock::new(source);
        b.extras = extras;
        Block::Image(b)
    } else {
        let mut b = DocumentBlock::new(source);
        b.extras = extras;
        Block::Document(b)
    }
}

fn is_image(mime: Option<&str>) -> bool {
    mime.is_some_and(|m| m.to_ascii_lowercase().starts_with("image/"))
}

fn file_source(origin: &Origin, uri: &str) -> Source {
    if uri.starts_with("files/") || uri.starts_with(FILES_URL) {
        Source::file_id(origin.clone(), uri)
    } else {
        Source::Url(uri.to_owned())
    }
}

fn blob_block(blob: Ext<BlobW>, mut extras: Extras) -> Block {
    let Ext {
        known,
        extra,
        verbatim,
    } = blob;
    add_nested_extra(&mut extras, INLINE, extra);
    verbatim_into_form(&mut extras, INLINE, verbatim);
    let image = is_image(Some(&known.mime_type));
    source_block(Source::base64(known.mime_type, known.data), image, extras)
}

fn file_block(origin: &Origin, file: Ext<FileDataW>, mut extras: Extras) -> Block {
    let Ext {
        known,
        extra,
        verbatim,
    } = file;
    add_nested_extra(&mut extras, FILE, extra);
    verbatim_into_form(&mut extras, FILE, verbatim);
    if let Some(mime) = &known.mime_type {
        extras
            .form
            .insert(FORM_MIME.to_owned(), mime.clone().into());
    }
    let image = is_image(known.mime_type.as_deref());
    source_block(file_source(origin, &known.file_uri), image, extras)
}

/// Function calls of the conversation so far, so a response without an `id` finds the call
/// it answers and every result knows the name Gemini wants back.
#[derive(Debug, Default)]
pub(crate) struct Calls {
    open: Vec<(String, String)>,
    named: Vec<(String, String)>,
}

impl Calls {
    pub(crate) fn name_of(&self, id: &str) -> Option<&str> {
        self.named
            .iter()
            .rev()
            .find(|(i, _)| i == id)
            .map(|(_, n)| n.as_str())
    }

    fn opened(&mut self, id: &str, name: &str) {
        self.open.push((id.to_owned(), name.to_owned()));
        self.named.push((id.to_owned(), name.to_owned()));
    }

    fn answered(&mut self, id: &str) {
        if let Some(i) = self.open.iter().position(|(o, _)| o == id) {
            self.open.remove(i);
        }
    }

    fn take_by_name(&mut self, name: &str) -> Option<String> {
        let i = self.open.iter().position(|(_, n)| n == name)?;
        Some(self.open.remove(i).0)
    }
}

pub(crate) struct PartDecoder<'a> {
    pub(crate) origin: &'a Origin,
    pub(crate) calls: &'a mut Calls,
    pub(crate) id_seed: String,
    pub(crate) generated: usize,
}

impl PartDecoder<'_> {
    fn next_id(&mut self) -> String {
        let id = format!("{GENERATED_ID_PREFIX}{}_{}", self.id_seed, self.generated);
        self.generated += 1;
        id
    }

    fn unknown(&self, raw: &Raw) -> Block {
        Block::Unknown(RawItem::new(
            self.origin.clone(),
            RawJson::from(raw.as_ref()),
        ))
    }

    /// The blocks of one part: a reasoning block that carries the part's thought signature,
    /// when it has one and isn't a thought itself, then the part's own block.
    pub(crate) fn part(&mut self, raw: &Raw, out: &mut Vec<Block>) -> Result<(), String> {
        let Ok(part) = Ext::<PartW>::from_raw(raw) else {
            out.push(self.unknown(raw));
            return Ok(());
        };
        let Ext {
            known: p,
            extra,
            verbatim,
        } = part;
        let data = [
            p.text.is_some(),
            p.inline_data.is_some(),
            p.file_data.is_some(),
            p.function_call.is_some(),
            p.function_response.is_some(),
        ]
        .into_iter()
        .filter(|d| *d)
        .count();
        let audio_video = p
            .inline_data
            .as_ref()
            .is_some_and(|b| is_media_without_block(&b.mime_type))
            || p.file_data
                .as_ref()
                .and_then(|f| f.mime_type.as_deref())
                .is_some_and(is_media_without_block);
        let thought = p.thought == Some(true);
        if data != 1 || audio_video || (thought && p.text.is_none()) {
            out.push(self.unknown(raw));
            return Ok(());
        }
        let mut extras = extras_from(self.origin, extra, verbatim);
        if thought {
            let mut block = ReasoningBlock::new(self.origin.clone());
            let text = p.text.unwrap_or_default();
            if text.is_empty() {
                extras
                    .form
                    .insert(FORM_THOUGHT_PART.to_owned(), true.into());
            } else {
                block.text.push(text);
            }
            if let Some(sig) = &p.thought_signature {
                block.opaque.push(signature(self.origin, sig));
            }
            block.extras = extras;
            out.push(Block::Reasoning(block));
            return Ok(());
        }
        if p.thought == Some(false) {
            extras.form.insert(FORM_THOUGHT.to_owned(), false.into());
        }
        if let Some(sig) = &p.thought_signature {
            out.push(Block::Reasoning(carrier(self.origin, sig)));
        }
        let block = if let Some(text) = p.text {
            let mut t = TextBlock::new(text);
            t.extras = extras;
            Block::Text(t)
        } else if let Some(blob) = p.inline_data {
            blob_block(blob, extras)
        } else if let Some(file) = p.file_data {
            file_block(self.origin, file, extras)
        } else if let Some(call) = p.function_call {
            self.call(call, extras)
        } else if let Some(response) = p.function_response {
            self.response(response, extras)?
        } else {
            return Ok(());
        };
        out.push(block);
        Ok(())
    }

    fn call(&mut self, call: Ext<FunctionCallW>, mut extras: Extras) -> Block {
        let Ext {
            known,
            extra,
            verbatim,
        } = call;
        add_nested_extra(&mut extras, CALL, extra);
        verbatim_into_form(&mut extras, CALL, verbatim);
        let id = match known.id {
            Some(id) => id,
            None => {
                extras.form.insert(FORM_NO_ID.to_owned(), true.into());
                self.next_id()
            }
        };
        let arguments = match known.args {
            Some(args) => RawJson::from(args),
            None => {
                extras.form.insert(FORM_NO_ARGS.to_owned(), true.into());
                RawJson::empty_object()
            }
        };
        self.calls.opened(&id, &known.name);
        let mut block = ToolCallBlock::new(id, known.name, arguments);
        block.extras = extras;
        Block::ToolCall(block)
    }

    fn response(
        &mut self,
        response: Ext<FunctionResponseW>,
        mut extras: Extras,
    ) -> Result<Block, String> {
        let Ext {
            known,
            extra,
            verbatim,
        } = response;
        add_nested_extra(&mut extras, RESPONSE, extra);
        verbatim_into_form(&mut extras, RESPONSE, verbatim);
        let id = match known.id {
            Some(id) => {
                self.calls.answered(&id);
                id
            }
            None => {
                extras.form.insert(FORM_NO_ID.to_owned(), true.into());
                match self.calls.take_by_name(&known.name) {
                    Some(id) => id,
                    None => self.next_id(),
                }
            }
        };
        if self.calls.name_of(&id) != Some(known.name.as_str()) {
            extras
                .form
                .insert(FORM_NAME.to_owned(), known.name.clone().into());
        }
        let mut content = Vec::new();
        let mut is_error = false;
        match known.response {
            None => {
                extras
                    .form
                    .insert(FORM_RESPONSE.to_owned(), "absent".into());
            }
            Some(raw) => match single_text(&raw) {
                Some(("output", text)) => content.push(Block::text(text)),
                Some(("error", text)) => {
                    is_error = true;
                    content.push(Block::text(text));
                }
                _ => {
                    extras.form.insert(FORM_RESPONSE.to_owned(), "json".into());
                    content.push(Block::text(compact(raw.get())));
                }
            },
        }
        for part in &known.parts {
            content.push(self.response_part(part));
        }
        let mut result = ToolResultBlock::new(id, content);
        result.is_error = is_error;
        result.extras = extras;
        Ok(Block::ToolResult(result))
    }

    fn response_part(&self, raw: &Raw) -> Block {
        let Ok(part) = Ext::<ResponsePartW>::from_raw(raw) else {
            return self.unknown(raw);
        };
        let Ext {
            known,
            extra,
            verbatim,
        } = part;
        let extras = extras_from(self.origin, extra, verbatim);
        match (known.inline_data, known.file_data) {
            (Some(blob), None) if !is_media_without_block(&blob.mime_type) => {
                blob_block(blob, extras)
            }
            (None, Some(file))
                if !file
                    .mime_type
                    .as_deref()
                    .is_some_and(is_media_without_block) =>
            {
                file_block(self.origin, file, extras)
            }
            _ => self.unknown(raw),
        }
    }
}

// The JSON without the whitespace between its tokens, its keys and numbers as written.
fn compact(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    let mut in_string = false;
    let mut escaped = false;
    for c in json.chars() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            _ if !in_string && c.is_ascii_whitespace() => continue,
            _ => {}
        }
        out.push(c);
    }
    out
}

// `{"output": "<text>"}` and `{"error": "<text>"}`, the shapes `FunctionResponse.response`
// documents for a function's output and its error.
fn single_text(raw: &Raw) -> Option<(&'static str, String)> {
    let map: serde_json::Map<String, Value> = serde_json::from_str(raw.get()).ok()?;
    let mut entries = map.into_iter();
    let (key, value) = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    let Value::String(text) = value else {
        return None;
    };
    let key = match key.as_str() {
        "output" => "output",
        "error" => "error",
        _ => return None,
    };
    let canonical = serde_json::to_string(&serde_json::json!({ key: text })).ok()?;
    let reparsed: Value = serde_json::from_str(&canonical).ok()?;
    let original: Value = serde_json::from_str(raw.get()).ok()?;
    (reparsed == original).then_some((key, text))
}

/// Writes blocks as the parts of one content.
pub(crate) struct PartEncoder<'a> {
    pub(crate) target: &'a Origin,
    pub(crate) pending: Option<String>,
    pub(crate) parts: Vec<Value>,
    pub(crate) reply: bool,
}

fn ser(e: serde_json::Error) -> KppError {
    KppError::provider(format!(
        "Kaeso could not write a Gemini part ({e}). This is a bug in Kaeso; please report it."
    ))
}

fn to_value<T: serde::Serialize>(v: &T) -> Result<Value, KppError> {
    serde_json::to_value(v).map_err(ser)
}

fn own_ext<T>(known: T, extras: &Extras, prefix: &str) -> Ext<T> {
    if prefix.is_empty() {
        Ext::with(
            known,
            extra_for(extras, D),
            verbatim_from_form(extras, D, ""),
        )
    } else {
        Ext::with(
            known,
            nested_extra_for(extras, D, prefix),
            verbatim_from_form(extras, D, prefix),
        )
    }
}

impl<'a> PartEncoder<'a> {
    pub(crate) fn new(target: &'a Origin) -> Self {
        PartEncoder {
            target,
            pending: None,
            parts: Vec::new(),
            reply: false,
        }
    }

    pub(crate) fn finish(mut self) -> Vec<Value> {
        if let Some(sig) = self.pending.take() {
            self.parts
                .push(serde_json::json!({"text": "", "thoughtSignature": sig}));
        }
        self.parts
    }

    fn push(&mut self, part: PartW, extras: &Extras) -> Result<(), KppError> {
        let mut part = part;
        if part.thought_signature.is_none() {
            part.thought_signature = self.pending.take();
        }
        if extras.form_value(D, FORM_THOUGHT) == Some(&Value::Bool(false)) {
            part.thought = Some(false);
        }
        let ext = own_ext(part, extras, "");
        self.parts.push(to_value(&ext)?);
        Ok(())
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn block(&mut self, block: &Block, path: &str) -> Result<(), KppError> {
        match block {
            Block::Text(t) => self.push(
                PartW {
                    text: Some(t.text.clone()),
                    ..PartW::default()
                },
                &t.extras,
            ),
            Block::Refusal(r) => self.push(
                PartW {
                    text: Some(r.text.clone()),
                    ..PartW::default()
                },
                &r.extras,
            ),
            Block::Reasoning(r) => self.reasoning(r),
            Block::Image(b) => self.media(&b.source, true, &b.extras, path),
            Block::Document(b) => self.media(&b.source, false, &b.extras, path),
            Block::ToolCall(c) => self.call(c, None),
            Block::ToolResult(r) => self.result(r, None, path),
            Block::Unknown(item) if self.reply && !item.origin.is(D) => Ok(()),
            Block::Unknown(item) => {
                if let Some(sig) = self.pending.take() {
                    self.parts
                        .push(serde_json::json!({"text": "", "thoughtSignature": sig}));
                }
                self.parts.push(to_value(&item.raw)?);
                Ok(())
            }
            other => Err(
                KppError::unsupported_by(format!("{} block", other.kind()), D, "").with_param(path),
            ),
        }
    }

    fn reasoning(&mut self, r: &ReasoningBlock) -> Result<(), KppError> {
        if !reasoning_replayable(r, self.target) {
            return Ok(());
        }
        if let Some(sig) = carried_signature(r, self.target) {
            if let Some(earlier) = self.pending.replace(sig.to_owned()) {
                self.parts
                    .push(serde_json::json!({"text": "", "thoughtSignature": earlier}));
            }
            return Ok(());
        }
        if let Some(earlier) = self.pending.take() {
            self.parts
                .push(serde_json::json!({"text": "", "thoughtSignature": earlier}));
        }
        let part = PartW {
            text: Some(r.text.concat()),
            thought: Some(true),
            thought_signature: thought_signature(r).map(str::to_owned),
            ..PartW::default()
        };
        let ext = own_ext(part, &r.extras, "");
        self.parts.push(to_value(&ext)?);
        Ok(())
    }

    fn media(
        &mut self,
        source: &Source,
        image: bool,
        extras: &Extras,
        path: &str,
    ) -> Result<(), KppError> {
        let part = media_part(source, image, extras, self.target, path)?;
        self.push(part, extras)
    }

    pub(crate) fn call(&mut self, c: &ToolCallBlock, dummy: Option<&str>) -> Result<(), KppError> {
        let own = c.extras.is_from(D);
        let omit_id =
            (own && c.extras.form_flag(D, FORM_NO_ID)) || c.id.starts_with(GENERATED_ID_PREFIX);
        let args = if own && c.extras.form_flag(D, FORM_NO_ARGS) && c.arguments.get() == "{}" {
            None
        } else {
            Some(c.arguments.clone().into_inner())
        };
        let call = own_ext(
            FunctionCallW {
                id: (!omit_id).then(|| c.id.clone()),
                name: c.name.clone(),
                args,
            },
            &c.extras,
            CALL,
        );
        let mut part = PartW {
            function_call: Some(call),
            ..PartW::default()
        };
        if self.pending.is_none() {
            part.thought_signature = dummy.map(str::to_owned);
        }
        self.push(part, &c.extras)
    }

    pub(crate) fn result(
        &mut self,
        r: &ToolResultBlock,
        name: Option<&str>,
        path: &str,
    ) -> Result<(), KppError> {
        let own = r.extras.is_from(D);
        let name = match (own.then(|| r.extras.form_str(D, FORM_NAME)).flatten(), name) {
            (Some(n), _) | (None, Some(n)) => n.to_owned(),
            (None, None) => {
                return Err(KppError::invalid_request(format!(
                    "The tool result for `{}` answers no earlier tool call; Gemini needs the name of the function it answers.",
                    r.call_id
                ))
                .with_param(path));
            }
        };
        let omit_id = (own && r.extras.form_flag(D, FORM_NO_ID))
            || r.call_id.starts_with(GENERATED_ID_PREFIX);
        let form = if own {
            r.extras.form_str(D, FORM_RESPONSE)
        } else {
            None
        };
        let mut texts = Vec::new();
        let mut parts = Vec::new();
        for (i, b) in r.content.iter().enumerate() {
            let at = format!("{path}.content[{i}]");
            match b {
                Block::Text(t) => texts.push(t.text.as_str()),
                Block::Image(img) => parts.push(media_response_part(
                    &img.source,
                    true,
                    &img.extras,
                    self.target,
                    &at,
                )?),
                Block::Document(doc) => parts.push(media_response_part(
                    &doc.source,
                    false,
                    &doc.extras,
                    self.target,
                    &at,
                )?),
                Block::Unknown(item) if item.origin.is(D) => {
                    parts.push(item.raw.clone().into_inner());
                }
                other => {
                    return Err(KppError::unsupported_by(
                        format!("{} block in a tool result", other.kind()),
                        D,
                        "Gemini function responses hold text, images and documents.",
                    )
                    .with_param(at));
                }
            }
        }
        let response = match form {
            Some("absent") => None,
            Some("json") => {
                let text = texts.concat();
                Some(match RawJson::from_string(text.clone()) {
                    Ok(raw) if raw.is_object() => raw.into_inner(),
                    _ => to_raw(&serde_json::json!({ "output": text }))?,
                })
            }
            _ => {
                let key = if r.is_error { "error" } else { "output" };
                Some(to_raw(&serde_json::json!({ key: texts.concat() }))?)
            }
        };
        let response = own_ext(
            FunctionResponseW {
                id: (!omit_id).then(|| r.call_id.clone()),
                name,
                response,
                parts,
            },
            &r.extras,
            RESPONSE,
        );
        self.push(
            PartW {
                function_response: Some(response),
                ..PartW::default()
            },
            &r.extras,
        )
    }
}

fn to_raw<T: serde::Serialize>(v: &T) -> Result<Raw, KppError> {
    serde_json::value::to_raw_value(v).map_err(ser)
}

type Media = (Option<Ext<BlobW>>, Option<Ext<FileDataW>>);

fn media_fields(
    source: &Source,
    image: bool,
    extras: &Extras,
    target: &Origin,
    path: &str,
) -> Result<Media, KppError> {
    let own = extras.is_from(D);
    let kind = if image { "image" } else { "document" };
    Ok(match source {
        Source::Base64(b) => (
            Some(own_ext(
                BlobW {
                    mime_type: b.media_type.clone(),
                    data: b.data.clone(),
                },
                extras,
                INLINE,
            )),
            None,
        ),
        Source::Text(t) => (
            Some(own_ext(
                BlobW {
                    mime_type: t.media_type.clone(),
                    data: base64::engine::general_purpose::STANDARD.encode(t.data.as_bytes()),
                },
                extras,
                INLINE,
            )),
            None,
        ),
        Source::Url(url) => {
            if crate::wire::is_data_url(url) {
                let Some(d) = crate::wire::parse_data_url(url) else {
                    return Err(KppError::unsupported_by(
                        format!("{kind} data URL that is not base64"),
                        D,
                        "Send the file as base64.",
                    )
                    .with_param(path));
                };
                (
                    Some(own_ext(
                        BlobW {
                            mime_type: d.media_type.to_owned(),
                            data: d.data.to_owned(),
                        },
                        extras,
                        INLINE,
                    )),
                    None,
                )
            } else {
                let mime = if own {
                    extras.form_str(D, FORM_MIME).map(str::to_owned)
                } else {
                    match (mime_of_url(url), image) {
                        (Some(m), _) => Some(m.to_owned()),
                        (None, false) => None,
                        (None, true) => {
                            return Err(KppError::unsupported_by(
                                "image URL without a known file extension",
                                D,
                                "Gemini needs an image URL's MIME type, which Kaeso reads from its extension (`.png`, `.jpg`, `.webp`, `.gif`, `.heic`); send the image as base64 instead.",
                            )
                            .with_param(path));
                        }
                    }
                };
                (
                    None,
                    Some(own_ext(
                        FileDataW {
                            mime_type: mime,
                            file_uri: url.clone(),
                        },
                        extras,
                        FILE,
                    )),
                )
            }
        }
        Source::FileId(f) if crate::cross::file_id_usable(&f.origin, target) => {
            let mime = if own {
                extras.form_str(D, FORM_MIME).map(str::to_owned)
            } else {
                None
            };
            (
                None,
                Some(own_ext(
                    FileDataW {
                        mime_type: mime,
                        file_uri: f.id.clone(),
                    },
                    extras,
                    FILE,
                )),
            )
        }
        _ => {
            return Err(KppError::unsupported_by(
                format!("{kind} source"),
                D,
                "Send the file as base64 or as a URL.",
            )
            .with_param(path));
        }
    })
}

/// The MIME type of a URL's file extension, for the types Gemini reads from `fileData`
/// ("File input methods", external URLs: "Always provide the correct MIME type").
fn mime_of_url(url: &str) -> Option<&'static str> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "heic" => "image/heic",
        "heif" => "image/heif",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        _ => return None,
    })
}

fn media_part(
    source: &Source,
    image: bool,
    extras: &Extras,
    target: &Origin,
    path: &str,
) -> Result<PartW, KppError> {
    let (inline_data, file_data) = media_fields(source, image, extras, target, path)?;
    Ok(PartW {
        inline_data,
        file_data,
        ..PartW::default()
    })
}

// Gemini 3 takes images and documents inside a function response ("Multimodal function
// responses", function calling guide); the API refuses other MIME types with a 400.
fn media_response_part(
    source: &Source,
    image: bool,
    extras: &Extras,
    target: &Origin,
    path: &str,
) -> Result<Raw, KppError> {
    let (inline_data, file_data) = media_fields(source, image, extras, target, path)?;
    let part = own_ext(
        ResponsePartW {
            inline_data,
            file_data,
        },
        extras,
        "",
    );
    to_raw(&part)
}
