//! Input items and content parts: what each becomes in KPP, and how KPP blocks are written back.

use kaeso_kpp::{
    Block, CacheHint, Extras, ImageBlock, JsonBlock, KppError, Message, Origin, RawItem, RawJson,
    RefusalBlock, Role, Source, TextBlock, ToolCallBlock, ToolResultBlock,
};
use serde_json::Value;
use serde_json::value::RawValue;

use super::DIALECT;
use super::wire::{
    CallOutputWire, CustomCallWire, FunctionCallWire, ImagePartWire, JsonPartWire, MessageWire,
    RefusalPartWire, TextPartWire,
};
use crate::wire::{
    Ext, extra_for, extras_from, parse_data_url, to_body, type_tag, verbatim_from_form,
    without_position,
};

/// Marks a message whose content was a string.
const FORM_CONTENT_STRING: &str = "content_string";
/// Marks a message written with `"type": "message"`.
const FORM_TYPED: &str = "typed";
/// The role a system message was sent with, `developer`.
const FORM_ROLE: &str = "role";
/// Marks a custom tool call or its output.
const FORM_CUSTOM: &str = "custom";
/// Marks a tool output that was a string.
const FORM_OUTPUT_STRING: &str = "output_string";
/// A text part's type where its role writes the other one.
const FORM_TEXT_TYPE: &str = "text_type";
/// A cache breakpoint as the app wrote it.
const FORM_BREAKPOINT: &str = "breakpoint";
/// The header of a data URL that isn't `data:<type>;base64`.
const FORM_DATA_URL_HEADER: &str = "data_url_header";

pub(super) const DEVELOPER: &str = "developer";

fn invalid(message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

fn unsupported(feature: &str, message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::unsupported(feature, message.into()).with_param(param)
}

pub(super) fn json(text: String) -> Result<RawJson, KppError> {
    RawJson::from_string(text).map_err(|e| {
        KppError::provider(format!(
            "Kaeso wrote an OpenAI Decisions value that is not JSON ({e}). This is a bug in Kaeso; please report it."
        ))
    })
}

pub(super) fn ser<T: serde::Serialize>(value: &T) -> Result<RawJson, KppError> {
    json(
        String::from_utf8(to_body(value)?)
            .map_err(|_| KppError::provider("Kaeso wrote a value that is not UTF-8."))?,
    )
}

/// Where a part stands, which decides the text part type it is written with and what it may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
    /// A user message: text, images and JSON.
    User,
    /// A developer or system message: text only.
    System,
    /// An assistant message: output text and refusals.
    Assistant,
    /// A tool output: text and images.
    ToolOutput,
    /// Question instructions: text, images and JSON.
    Instructions,
}

impl Place {
    fn text_type(self) -> &'static str {
        match self {
            Place::Assistant => "output_text",
            _ => "input_text",
        }
    }

    fn what(self) -> &'static str {
        match self {
            Place::User => "a user message",
            Place::System => "a developer or system message",
            Place::Assistant => "an assistant message",
            Place::ToolOutput => "a tool output",
            Place::Instructions => "question instructions",
        }
    }

    fn takes(self) -> &'static str {
        match self {
            Place::User | Place::Instructions => "text, images and JSON",
            Place::System => "text",
            Place::Assistant => "output text and refusals",
            Place::ToolOutput => "text and images",
        }
    }
}

fn breakpoint_of(raw: Option<RawJson>, origin: &Origin) -> Option<CacheHint> {
    let raw = raw?;
    let mut hint = CacheHint::new(None);
    hint.extras = Extras::new(origin.clone())
        .with_form(FORM_BREAKPOINT, raw.to_value().unwrap_or(Value::Null));
    Some(hint)
}

fn breakpoint_for(cache: Option<&CacheHint>) -> Result<Option<RawJson>, KppError> {
    match cache {
        None => Ok(None),
        Some(hint) => match hint.extras.form_value(DIALECT, FORM_BREAKPOINT) {
            Some(v) => ser(v).map(Some),
            None => json(r#"{"mode":"explicit"}"#.to_owned()).map(Some),
        },
    }
}

/// Reads one content part; a part of a type the Decisions API doesn't document is kept whole,
/// bound to this format.
pub(super) fn decode_part(
    raw: &RawValue,
    place: Place,
    origin: &Origin,
    param: &str,
) -> Result<Block, KppError> {
    let ty = type_tag(raw).ok().flatten();
    let bad = |what: &str, e: serde_json::Error| {
        invalid(
            format!("The `{what}` part is not valid: {}.", without_position(&e)),
            param,
        )
    };
    let block = match ty.as_deref() {
        Some(t @ ("input_text" | "output_text")) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<TextPartWire>::from_raw(raw).map_err(|e| bad(t, e))?;
            let mut b = TextBlock::new(w.text);
            b.cache = breakpoint_of(w.prompt_cache_breakpoint, origin);
            b.extras = extras_from(origin, extra, verbatim);
            if t != place.text_type() {
                b.extras.form.insert(FORM_TEXT_TYPE.to_owned(), t.into());
            }
            Block::Text(b)
        }
        Some("input_image") => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<ImagePartWire>::from_raw(raw).map_err(|e| bad("input_image", e))?;
            let mut extras = extras_from(origin, extra, verbatim);
            let source = match parse_data_url(&w.image_url) {
                Some(data) => {
                    if let Some(header) = data.header {
                        extras
                            .form
                            .insert(FORM_DATA_URL_HEADER.to_owned(), header.into());
                    }
                    crate::media::from_base64(
                        data.media_type,
                        data.data,
                        &format!("{param}.image_url"),
                    )?
                }
                None => Source::Url(w.image_url),
            };
            let mut b = ImageBlock::new(source);
            b.cache = breakpoint_of(w.prompt_cache_breakpoint, origin);
            b.extras = extras;
            Block::Image(b)
        }
        Some("input_json") => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<JsonPartWire>::from_raw(raw).map_err(|e| bad("input_json", e))?;
            let mut b = JsonBlock::new(w.data);
            b.extras = extras_from(origin, extra, verbatim);
            Block::Json(b)
        }
        Some("refusal") => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<RefusalPartWire>::from_raw(raw).map_err(|e| bad("refusal", e))?;
            let mut b = RefusalBlock::new(w.refusal);
            b.extras = extras_from(origin, extra, verbatim);
            Block::Refusal(b)
        }
        _ => Block::Unknown(RawItem::new(origin.clone(), RawJson::from(raw))),
    };
    Ok(block)
}

/// Reads a content string or a list of parts.
pub(super) fn decode_content(
    raw: &RawJson,
    place: Place,
    origin: &Origin,
    param: &str,
    extras: &mut Extras,
    string_form: &str,
) -> Result<Vec<Block>, KppError> {
    if raw.get().starts_with('"') {
        let text: String = raw
            .parse()
            .map_err(|_| invalid("The content is not a valid string.", param))?;
        extras.form.insert(string_form.to_owned(), true.into());
        return Ok(vec![Block::text(text)]);
    }
    let parts: Vec<&RawValue> = raw.parse().map_err(|_| {
        invalid(
            format!(
                "The content of {} must be a string or a list of parts.",
                place.what()
            ),
            param,
        )
    })?;
    parts
        .into_iter()
        .enumerate()
        .map(|(i, p)| decode_part(p, place, origin, &format!("{param}[{i}]")))
        .collect()
}

/// Reads one input item: a message, or a function or custom tool call or output.
pub(super) fn decode_item(raw: &RawValue, origin: &Origin, i: usize) -> Result<Message, KppError> {
    let param = format!("input[{i}]");
    if !raw.get().starts_with('{') {
        return Err(invalid("Every input item must be an object.", param));
    }
    let ty = type_tag(raw).ok().flatten();
    let bad = |what: &str, e: serde_json::Error| {
        invalid(
            format!("The `{what}` item is not valid: {}.", without_position(&e)),
            param.clone(),
        )
    };
    match ty.as_deref() {
        None | Some("message") => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<MessageWire>::from_raw(raw).map_err(|e| bad("message", e))?;
            let mut extras = extras_from(origin, extra, verbatim);
            if w.ty.is_some() {
                extras.form.insert(FORM_TYPED.to_owned(), true.into());
            }
            let (role, place) = match w.role.as_str() {
                "user" => (Role::User, Place::User),
                "assistant" => (Role::Assistant, Place::Assistant),
                "system" => (Role::System, Place::System),
                DEVELOPER => {
                    extras.form.insert(FORM_ROLE.to_owned(), DEVELOPER.into());
                    (Role::System, Place::System)
                }
                other => {
                    return Err(invalid(
                        format!(
                            "`{other}` is not a message role the Decisions API takes; use `user`, `developer`, `system` or `assistant`."
                        ),
                        format!("{param}.role"),
                    ));
                }
            };
            let blocks = decode_content(
                &w.content,
                place,
                origin,
                &format!("{param}.content"),
                &mut extras,
                FORM_CONTENT_STRING,
            )?;
            let mut m = Message::new(role, blocks);
            m.extras = extras;
            Ok(m)
        }
        Some(t @ ("function_call" | "custom_tool_call")) => {
            let custom = t == "custom_tool_call";
            let (call_id, name, text, extra, verbatim) = if custom {
                let Ext {
                    known: w,
                    extra,
                    verbatim,
                } = Ext::<CustomCallWire>::from_raw(raw).map_err(|e| bad(t, e))?;
                (w.call_id, w.name, w.input, extra, verbatim)
            } else {
                let Ext {
                    known: w,
                    extra,
                    verbatim,
                } = Ext::<FunctionCallWire>::from_raw(raw).map_err(|e| bad(t, e))?;
                (w.call_id, w.name, w.arguments, extra, verbatim)
            };
            let arguments =
                RawJson::from_serialize(&text).map_err(|e| KppError::provider(e.to_string()))?;
            let mut call = ToolCallBlock::new(call_id, name, arguments);
            call.extras = extras_from(origin, extra, verbatim);
            if custom {
                call.extras.form.insert(FORM_CUSTOM.to_owned(), true.into());
            }
            Ok(Message::new(Role::Assistant, vec![Block::ToolCall(call)]))
        }
        Some(t @ ("function_call_output" | "custom_tool_call_output")) => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<CallOutputWire>::from_raw(raw).map_err(|e| bad(t, e))?;
            let mut extras = extras_from(origin, extra, verbatim);
            let content = decode_content(
                &w.output,
                Place::ToolOutput,
                origin,
                &format!("{param}.output"),
                &mut extras,
                FORM_OUTPUT_STRING,
            )?;
            if t == "custom_tool_call_output" {
                extras.form.insert(FORM_CUSTOM.to_owned(), true.into());
            }
            let mut result = ToolResultBlock::new(w.call_id, content);
            result.extras = extras;
            Ok(Message::new(Role::User, vec![Block::ToolResult(result)]))
        }
        Some(other) => Err(invalid(
            format!(
                "`{other}` is not an input item the Decisions API takes; send messages, function calls, custom tool calls and their outputs."
            ),
            format!("{param}.type"),
        )),
    }
}

/// What the encoder writes a part with: images as data URLs, or, checking only, without their
/// bytes; JSON that doesn't fit `input_json` as text.
pub(super) struct PartWriter<'a> {
    pub dry: bool,
    pub adapted: &'a mut Vec<&'static str>,
    /// Whether this place may hold images.
    pub images: bool,
    /// The provider's name in refusals.
    pub name: &'static str,
}

impl PartWriter<'_> {
    fn note(&mut self, what: &'static str) {
        if !self.adapted.contains(&what) {
            self.adapted.push(what);
        }
    }

    pub(super) fn image_url(&self, b: &ImageBlock, param: &str) -> Result<String, KppError> {
        match &b.source {
            Source::Url(url) => {
                let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
                if lower.starts_with("https://")
                    || lower.starts_with("http://")
                    || crate::wire::is_data_url(url)
                {
                    Ok(url.clone())
                } else {
                    Err(unsupported(
                        "image URL",
                        format!(
                            "{} takes images as http(s) URLs or data URLs, not this URL; send the image as a data URL.",
                            self.name
                        ),
                        param,
                    ))
                }
            }
            Source::Base64(s) if self.dry => Ok(format!("data:{};base64,", s.media_type)),
            Source::Base64(s) => Ok(match b.extras.form_str(DIALECT, FORM_DATA_URL_HEADER) {
                Some(header) => s.data_url_with(header),
                None => s.data_url(),
            }),
            Source::FileId(_) => Err(unsupported(
                "image file id",
                format!(
                    "{} takes no file ids in decision requests; send the image as a data URL.",
                    self.name
                ),
                param,
            )),
            _ => Err(unsupported(
                "image source",
                format!("{} takes images only as URLs or data URLs.", self.name),
                param,
            )),
        }
    }

    pub(super) fn image(&mut self, b: &ImageBlock, param: &str) -> Result<RawJson, KppError> {
        if !self.images {
            return Err(unsupported(
                "image in question instructions",
                format!(
                    "{} takes no images in question instructions; put the image in the input.",
                    self.name
                ),
                param,
            ));
        }
        let w = ImagePartWire {
            ty: "input_image".to_owned(),
            image_url: self.image_url(b, param)?,
            prompt_cache_breakpoint: breakpoint_for(b.cache.as_ref())?,
        };
        ser(&Ext::with(
            w,
            extra_for(&b.extras, DIALECT),
            verbatim_from_form(&b.extras, DIALECT, ""),
        ))
    }

    pub(super) fn text_part(&self, text: &str) -> Result<RawJson, KppError> {
        ser(&TextPartWire {
            ty: "input_text".to_owned(),
            text: text.to_owned(),
            prompt_cache_breakpoint: None,
        })
    }

    /// JSON as an `input_json` part when it fits the format's limits, else as text.
    pub(super) fn json_value(&mut self, data: &RawJson) -> Result<RawJson, KppError> {
        if super::state::fits_input_json(data) {
            ser(&JsonPartWire {
                ty: "input_json".to_owned(),
                data: data.clone(),
            })
        } else {
            self.note("state");
            self.text_part(data.get())
        }
    }

    pub(super) fn part(
        &mut self,
        block: &Block,
        place: Place,
        param: &str,
    ) -> Result<RawJson, KppError> {
        let refuse = |kind: &str| {
            unsupported(
                &format!("{kind} part"),
                format!(
                    "{} takes {} in {}, not a {kind} part; remove it.",
                    self.name,
                    place.takes(),
                    place.what()
                ),
                param,
            )
        };
        match (block, place) {
            (Block::Text(t), _) => {
                let ty = t
                    .extras
                    .form_str(DIALECT, FORM_TEXT_TYPE)
                    .unwrap_or(place.text_type());
                let w = TextPartWire {
                    ty: ty.to_owned(),
                    text: t.text.clone(),
                    prompt_cache_breakpoint: breakpoint_for(t.cache.as_ref())?,
                };
                ser(&Ext::with(
                    w,
                    extra_for(&t.extras, DIALECT),
                    verbatim_from_form(&t.extras, DIALECT, ""),
                ))
            }
            (Block::Image(b), Place::User | Place::ToolOutput | Place::Instructions) => {
                self.image(b, param)
            }
            (Block::Json(j), Place::User | Place::Instructions) => {
                if j.extras.is_from(DIALECT) {
                    let w = JsonPartWire {
                        ty: "input_json".to_owned(),
                        data: j.data.clone(),
                    };
                    ser(&Ext::with(
                        w,
                        extra_for(&j.extras, DIALECT),
                        verbatim_from_form(&j.extras, DIALECT, ""),
                    ))
                } else {
                    self.json_value(&j.data)
                }
            }
            (Block::Refusal(r), Place::Assistant) => {
                let w = RefusalPartWire {
                    ty: "refusal".to_owned(),
                    refusal: r.text.clone(),
                };
                ser(&Ext::with(
                    w,
                    extra_for(&r.extras, DIALECT),
                    verbatim_from_form(&r.extras, DIALECT, ""),
                ))
            }
            (Block::Unknown(item), _) if item.origin.is(DIALECT) => Ok(item.raw.clone()),
            (other, _) => Err(refuse(other.kind())),
        }
    }

    fn parts(
        &mut self,
        blocks: &[Block],
        place: Place,
        param: &str,
    ) -> Result<Vec<RawJson>, KppError> {
        blocks
            .iter()
            .enumerate()
            .map(|(i, b)| self.part(b, place, &format!("{param}[{i}]")))
            .collect()
    }

    // A content string where the app wrote one, else the parts.
    fn content(
        &mut self,
        blocks: &[Block],
        place: Place,
        param: &str,
        extras: &Extras,
        string_form: &str,
    ) -> Result<RawJson, KppError> {
        if extras.form_flag(DIALECT, string_form)
            && let [Block::Text(t)] = blocks
            && t.cache.is_none()
            && t.extras.is_empty()
        {
            return ser(&t.text);
        }
        ser(&self.parts(blocks, place, param)?)
    }

    /// The input items of a list of messages: a message for each run of content, and an item of
    /// its own for each tool call and output.
    pub(super) fn items(&mut self, messages: &[Message]) -> Result<Vec<RawJson>, KppError> {
        let mut items = Vec::new();
        for (mi, m) in messages.iter().enumerate() {
            let param = format!("input[{mi}]");
            let mut run: Vec<Block> = Vec::new();
            let mut wrote = false;
            for (bi, block) in m.blocks.iter().enumerate() {
                let item = match block {
                    Block::ToolCall(call) => tool_call(call)?,
                    Block::ToolResult(result) => {
                        self.tool_output(result, &format!("{param}.blocks[{bi}]"))?
                    }
                    other => {
                        run.push(other.clone());
                        continue;
                    }
                };
                if !run.is_empty() {
                    items.push(self.message(m, &run, &param)?);
                    run.clear();
                }
                items.push(item);
                wrote = true;
            }
            if !run.is_empty() || !wrote {
                items.push(self.message(m, &run, &param)?);
            }
        }
        Ok(items)
    }

    fn message(&mut self, m: &Message, blocks: &[Block], param: &str) -> Result<RawJson, KppError> {
        let (role, place) = match m.role {
            Role::User => ("user", Place::User),
            Role::Assistant => ("assistant", Place::Assistant),
            Role::System if m.extras.form_str(DIALECT, FORM_ROLE) == Some(DEVELOPER) => {
                (DEVELOPER, Place::System)
            }
            Role::System => ("system", Place::System),
            _ => {
                return Err(unsupported(
                    "message role",
                    format!("{} can't take this message role.", self.name),
                    param,
                ));
            }
        };
        let content = self.content(
            blocks,
            place,
            &format!("{param}.content"),
            &m.extras,
            FORM_CONTENT_STRING,
        )?;
        let message = MessageWire {
            ty: m
                .extras
                .form_flag(DIALECT, FORM_TYPED)
                .then(|| "message".to_owned()),
            role: role.to_owned(),
            content,
        };
        ser(&Ext::with(
            message,
            extra_for(&m.extras, DIALECT),
            verbatim_from_form(&m.extras, DIALECT, ""),
        ))
    }

    fn tool_output(&mut self, r: &ToolResultBlock, param: &str) -> Result<RawJson, KppError> {
        if r.is_error {
            return Err(unsupported(
                "tool error",
                format!(
                    "{} has no way to mark a tool output as an error; send it as plain output.",
                    self.name
                ),
                param,
            ));
        }
        let custom = r.extras.form_flag(DIALECT, FORM_CUSTOM);
        let output = self.content(
            &r.content,
            Place::ToolOutput,
            &format!("{param}.output"),
            &r.extras,
            FORM_OUTPUT_STRING,
        )?;
        let w = CallOutputWire {
            ty: if custom {
                "custom_tool_call_output"
            } else {
                "function_call_output"
            }
            .to_owned(),
            call_id: r.call_id.clone(),
            output,
        };
        ser(&Ext::with(
            w,
            extra_for(&r.extras, DIALECT),
            verbatim_from_form(&r.extras, DIALECT, ""),
        ))
    }
}

fn tool_call(call: &ToolCallBlock) -> Result<RawJson, KppError> {
    let text = match call.arguments.parse::<String>() {
        Ok(s) if call.arguments.get().starts_with('"') => s,
        _ => call.arguments.get().to_owned(),
    };
    let extra = extra_for(&call.extras, DIALECT);
    let verbatim = verbatim_from_form(&call.extras, DIALECT, "");
    if call.extras.form_flag(DIALECT, FORM_CUSTOM) {
        let w = CustomCallWire {
            ty: "custom_tool_call".to_owned(),
            call_id: call.id.clone(),
            name: call.name.clone(),
            input: text,
        };
        ser(&Ext::with(w, extra, verbatim))
    } else {
        let w = FunctionCallWire {
            ty: "function_call".to_owned(),
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: text,
        };
        ser(&Ext::with(w, extra, verbatim))
    }
}
