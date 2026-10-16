use kaeso_kpp::{KppError, OpaqueKind};

use super::{Blk, Emit, Engine, NEW_PART, PartKind, ev, int, sc};
use crate::cross::{ReplyPolicy, reply_opaque_visible, reply_policy};
use crate::openai_responses::DIALECT;
use crate::openai_responses::node::Node;

impl Engine {
    pub(super) fn delta(
        &mut self,
        d: &kaeso_kpp::BlockDelta,
        em: &mut Emit<'_>,
    ) -> Result<(), KppError> {
        use kaeso_kpp::Delta;
        if self.hidden.contains(&d.index) {
            return Ok(());
        }
        let Some(mut blk) = self.blocks.remove(&d.index) else {
            return Err(KppError::provider(format!(
                "The stream sent a delta for block {} before it started. This is a bug in Kaeso; please report it.",
                d.index
            )));
        };
        let result = (|| -> Result<(), KppError> {
            match (&mut blk, &d.delta) {
                (
                    Blk::Part {
                        kind: PartKind::Text,
                        output_index,
                        item_id,
                        content_index,
                        text,
                        logprobs,
                        ..
                    },
                    Delta::Text(t),
                ) => {
                    let out = self.emit(
                        em,
                        "",
                        ev("response.output_text.delta")
                            .with("item_id", Node::str(item_id.as_str()))
                            .with("output_index", int(*output_index))
                            .with("content_index", int(*content_index))
                            .with("delta", Node::str(t))
                            .with("logprobs", Node::Arr(Vec::new())),
                    )?;
                    text.push_str(t);
                    if let Some(lp) = out.get("logprobs") {
                        logprobs.extend(lp.elements());
                    }
                }
                (
                    Blk::Part {
                        kind: PartKind::Text,
                        output_index,
                        item_id,
                        content_index,
                        annotations,
                        ..
                    },
                    Delta::Citation(c),
                ) => {
                    if reply_policy(Some(&c.origin), DIALECT) == ReplyPolicy::Keep {
                        let index = u64::try_from(annotations.len()).unwrap_or(u64::MAX);
                        let out = self.emit(
                            em,
                            "",
                            ev("response.output_text.annotation.added")
                                .with("item_id", Node::str(item_id.as_str()))
                                .with("output_index", int(*output_index))
                                .with("content_index", int(*content_index))
                                .with("annotation_index", Node::int(index))
                                .with("annotation", Node::raw(&c.raw)),
                        )?;
                        annotations
                            .push(out.get("annotation").unwrap_or_else(|| Node::raw(&c.raw)));
                    }
                }
                (
                    Blk::Part {
                        kind: PartKind::Refusal,
                        output_index,
                        item_id,
                        content_index,
                        text,
                        ..
                    },
                    Delta::Refusal(t) | Delta::Text(t),
                ) => {
                    self.emit(
                        em,
                        "",
                        ev("response.refusal.delta")
                            .with("item_id", Node::str(item_id.as_str()))
                            .with("output_index", int(*output_index))
                            .with("content_index", int(*content_index))
                            .with("delta", Node::str(t)),
                    )?;
                    text.push_str(t);
                }
                (
                    Blk::Call {
                        output_index,
                        id,
                        args,
                        ..
                    },
                    Delta::ToolArguments(t),
                ) => {
                    self.emit(
                        em,
                        "",
                        ev("response.function_call_arguments.delta")
                            .with("item_id", Node::str(id.as_str()))
                            .with("output_index", int(*output_index))
                            .with("delta", Node::str(t)),
                    )?;
                    args.push_str(t);
                }
                (Blk::Reasoning { carry: Some(_), .. }, delta) => {
                    self.carried_delta(&mut blk, delta, em)?;
                }
                (Blk::Reasoning { .. }, Delta::ReasoningSummary(s)) => {
                    self.summary_delta(&mut blk, s.summary_index, &s.text, em)?;
                }
                (Blk::Reasoning { .. }, Delta::ReasoningText(t)) => {
                    let new_part = d.extras.form_flag(DIALECT, NEW_PART);
                    self.reasoning_text_delta(&mut blk, t, new_part, em)?;
                }
                (Blk::Reasoning { encrypted, .. }, Delta::Opaque(o)) => {
                    if o.kind == OpaqueKind::OpenAiEncryptedReasoning
                        && reply_opaque_visible(o, DIALECT)
                    {
                        *encrypted = Some(o.data.clone());
                    }
                }
                (Blk::Unknown { last, .. }, Delta::Unknown(item)) if item.origin.is(DIALECT) => {
                    *last = Some(Node::raw(&item.raw));
                }
                _ => {}
            }
            Ok(())
        })();
        self.blocks.insert(d.index, blk);
        result
    }

    fn carried_delta(
        &mut self,
        blk: &mut Blk,
        delta: &kaeso_kpp::Delta,
        em: &mut Emit<'_>,
    ) -> Result<(), KppError> {
        use kaeso_kpp::Delta;
        let Blk::Reasoning {
            carry: Some(carry), ..
        } = blk
        else {
            return Ok(());
        };
        crate::continuity::add_reasoning_delta(carry, delta, false)?;
        let shown = match delta {
            Delta::ReasoningText(t) => Some((0, t)),
            Delta::ReasoningSummary(s) => Some((s.summary_index, &s.text)),
            _ => None,
        };
        match shown {
            Some((index, text)) if !text.is_empty() => {
                let text = text.clone();
                self.summary_delta(blk, index, &text, em)
            }
            _ => Ok(()),
        }
    }

    fn summary_delta(
        &mut self,
        blk: &mut Blk,
        index: u32,
        text: &str,
        em: &mut Emit<'_>,
    ) -> Result<(), KppError> {
        let Blk::Reasoning {
            output_index,
            id,
            summaries,
            open_summary,
            ..
        } = blk
        else {
            return Ok(());
        };
        let (oi, item_id) = (*output_index, id.clone());
        let base = |ty: &str, si: u32| {
            ev(ty)
                .with("item_id", Node::str(&item_id))
                .with("output_index", int(oi))
                .with("summary_index", int(si))
        };
        let slot = usize::try_from(index).unwrap_or(usize::MAX);
        let main = if *open_summary == Some(index) {
            ""
        } else {
            if let Some(prev) = open_summary.take() {
                self.close_summary(em, "", &item_id, oi, prev, summaries)?;
            }
            crate::continuity::summary_part(summaries, index)?;
            self.emit(
                em,
                "",
                base("response.reasoning_summary_part.added", index).with(
                    "part",
                    Node::obj()
                        .with("type", Node::str("summary_text"))
                        .with("text", Node::str("")),
                ),
            )?;
            *open_summary = Some(index);
            "@text"
        };
        if !text.is_empty() {
            self.emit(
                em,
                main,
                base("response.reasoning_summary_text.delta", index).with("delta", Node::str(text)),
            )?;
            if let Some(s) = summaries.get_mut(slot) {
                s.push_str(text);
            }
        }
        Ok(())
    }

    pub(super) fn close_summary(
        &mut self,
        em: &mut Emit<'_>,
        prefix: &str,
        item_id: &str,
        oi: u32,
        index: u32,
        summaries: &[String],
    ) -> Result<(), KppError> {
        let text = summaries
            .get(usize::try_from(index).unwrap_or(usize::MAX))
            .cloned()
            .unwrap_or_default();
        let base = |ty: &str| {
            ev(ty)
                .with("item_id", Node::str(item_id))
                .with("output_index", int(oi))
                .with("summary_index", int(index))
        };
        self.emit(
            em,
            &sc(prefix, "@sdone"),
            base("response.reasoning_summary_text.done").with("text", Node::str(&text)),
        )?;
        self.emit(
            em,
            &sc(prefix, "@spdone"),
            base("response.reasoning_summary_part.done").with(
                "part",
                Node::obj()
                    .with("type", Node::str("summary_text"))
                    .with("text", Node::str(&text)),
            ),
        )?;
        Ok(())
    }

    fn reasoning_text_delta(
        &mut self,
        blk: &mut Blk,
        text: &str,
        new_part: bool,
        em: &mut Emit<'_>,
    ) -> Result<(), KppError> {
        let Blk::Reasoning {
            output_index,
            id,
            texts,
            open_text,
            ..
        } = blk
        else {
            return Ok(());
        };
        let (oi, item_id) = (*output_index, id.clone());
        if !new_part && text.is_empty() {
            return Ok(());
        }
        let mut main = "";
        if new_part || open_text.is_none() {
            if let Some(prev) = open_text.take() {
                self.close_reasoning_text(em, "", &item_id, oi, prev, texts)?;
            }
            let ci = u32::try_from(texts.len()).unwrap_or(u32::MAX);
            texts.push(String::new());
            self.emit(
                em,
                "",
                ev("response.content_part.added")
                    .with("item_id", Node::str(&item_id))
                    .with("output_index", int(oi))
                    .with("content_index", int(ci))
                    .with(
                        "part",
                        Node::obj()
                            .with("type", Node::str("reasoning_text"))
                            .with("text", Node::str("")),
                    ),
            )?;
            *open_text = Some(ci);
            main = "@text";
        }
        if !text.is_empty() {
            let ci = open_text.unwrap_or(0);
            self.emit(
                em,
                main,
                ev("response.reasoning_text.delta")
                    .with("item_id", Node::str(&item_id))
                    .with("output_index", int(oi))
                    .with("content_index", int(ci))
                    .with("delta", Node::str(text)),
            )?;
            if let Some(t) = texts.last_mut() {
                t.push_str(text);
            }
        }
        Ok(())
    }

    pub(super) fn close_reasoning_text(
        &mut self,
        em: &mut Emit<'_>,
        prefix: &str,
        item_id: &str,
        oi: u32,
        index: u32,
        texts: &[String],
    ) -> Result<(), KppError> {
        let text = texts
            .get(usize::try_from(index).unwrap_or(usize::MAX))
            .cloned()
            .unwrap_or_default();
        let base = |ty: &str| {
            ev(ty)
                .with("item_id", Node::str(item_id))
                .with("output_index", int(oi))
                .with("content_index", int(index))
        };
        self.emit(
            em,
            &sc(prefix, "@rdone"),
            base("response.reasoning_text.done").with("text", Node::str(&text)),
        )?;
        self.emit(
            em,
            &sc(prefix, "@rpdone"),
            base("response.content_part.done").with(
                "part",
                Node::obj()
                    .with("type", Node::str("reasoning_text"))
                    .with("text", Node::str(&text)),
            ),
        )?;
        Ok(())
    }
}
