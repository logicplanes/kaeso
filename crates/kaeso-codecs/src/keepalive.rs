use kaeso_kpp::{Delta, DialectId, StreamEvent};

pub(crate) const COMMENT_FRAME: &[u8] = b": keepalive\n\n";

pub(crate) const PING_FRAME: &[u8] = b"event: ping\ndata: {\"type\": \"ping\"}\n\n";

pub const HIDDEN_TEXT_PER_KEEPALIVE: usize = 1024;

#[derive(Clone, Debug, Default)]
pub(crate) struct Pacer {
    hidden: usize,
}

impl Pacer {
    pub(crate) fn pace(
        &mut self,
        event: &StreamEvent,
        target: DialectId,
        out: &mut Vec<u8>,
        frame: &[u8],
    ) {
        if !out.is_empty() {
            self.hidden = 0;
            return;
        }
        if is_own(event, target) {
            return;
        }
        let weight = match event {
            StreamEvent::Extra(_) => HIDDEN_TEXT_PER_KEEPALIVE,
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::Text(t)
                | Delta::ToolArguments(t)
                | Delta::ReasoningText(t)
                | Delta::Refusal(t) => t.len(),
                Delta::ReasoningSummary(s) => s.text.len(),
                _ => 0,
            },
            _ => 0,
        };
        self.hidden = self.hidden.saturating_add(weight);
        if self.hidden >= HIDDEN_TEXT_PER_KEEPALIVE {
            self.hidden = 0;
            out.extend_from_slice(frame);
        }
    }

    pub(crate) fn reset(&mut self) {
        self.hidden = 0;
    }
}

fn is_own(event: &StreamEvent, dialect: DialectId) -> bool {
    let extras = match event {
        StreamEvent::Extra(item) => return item.origin.is(dialect),
        StreamEvent::ResponseStart(e) => &e.extras,
        StreamEvent::BlockStart(e) => &e.extras,
        StreamEvent::BlockDelta(e) => &e.extras,
        StreamEvent::BlockStop(e) => &e.extras,
        StreamEvent::ResponseDelta(e) => &e.extras,
        StreamEvent::ResponseStop(e) => &e.extras,
        _ => return false,
    };
    extras.is_from(dialect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{BlockDelta, Extras, RawItem, RawJson};

    const A: DialectId = DialectId::AnthropicMessages;
    const C: DialectId = DialectId::OpenAiChat;

    fn thinking(n: usize) -> StreamEvent {
        StreamEvent::BlockDelta(BlockDelta::new(0, Delta::ReasoningText("x".repeat(n))))
    }

    fn ping(origin: DialectId) -> StreamEvent {
        StreamEvent::Extra(RawItem::new(
            origin,
            RawJson::from_string(r#"{"event":"ping","data":{"type":"ping"}}"#.into()).unwrap(),
        ))
    }

    fn paced(p: &mut Pacer, ev: &StreamEvent) -> bool {
        let mut out = Vec::new();
        p.pace(ev, C, &mut out, COMMENT_FRAME);
        !out.is_empty()
    }

    #[test]
    fn hidden_text_earns_one_frame_per_threshold() {
        let mut p = Pacer::default();
        let frames = (0..100).filter(|_| paced(&mut p, &thinking(100))).count();
        assert_eq!(frames, 100 * 100 / HIDDEN_TEXT_PER_KEEPALIVE);
    }

    #[test]
    fn foreign_extra_events_always_earn_a_frame_own_events_never() {
        let mut p = Pacer::default();
        assert!(paced(&mut p, &ping(A)));
        assert!(paced(&mut p, &ping(A)));
        assert!(!paced(&mut p, &ping(C)));
        let mut own = BlockDelta::new(0, Delta::Text("x".repeat(5000)));
        own.extras = Extras::new(C);
        assert!(!paced(&mut p, &StreamEvent::BlockDelta(own)));
    }

    #[test]
    fn written_output_resets_the_count() {
        let mut p = Pacer::default();
        assert!(!paced(&mut p, &thinking(1000)));
        let mut out = b"data: {}\n\n".to_vec();
        p.pace(&thinking(1000), C, &mut out, COMMENT_FRAME);
        assert_eq!(out, b"data: {}\n\n");
        assert!(!paced(&mut p, &thinking(1000)));
        assert!(paced(&mut p, &thinking(24)));
    }
}
