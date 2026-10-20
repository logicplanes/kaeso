use kaeso_kpp::{Block, Delta, StreamEvent, Usage};

pub const BYTES_PER_TOKEN: u64 = 4;

pub fn tokens_of_bytes(bytes: u64) -> u64 {
    bytes.div_ceil(BYTES_PER_TOKEN)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    deltas: u64,
    bytes: u64,
    reasoning_deltas: u64,
    reasoning_bytes: u64,
    final_usage: bool,
}

impl Tally {
    pub fn new() -> Tally {
        Tally::default()
    }

    pub fn feed(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::Text(s) | Delta::ToolArguments(s) | Delta::Refusal(s) => {
                    self.add(s.len(), false);
                }
                Delta::ReasoningText(s) => self.add(s.len(), true),
                Delta::ReasoningSummary(s) => self.add(s.text.len(), true),
                _ => {}
            },
            StreamEvent::ResponseDelta(d) if d.usage.is_some() => self.final_usage = true,
            _ => {}
        }
    }

    pub fn feed_blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            match block {
                Block::Text(t) => self.add(t.text.len(), false),
                Block::Refusal(r) => self.add(r.text.len(), false),
                Block::ToolCall(c) => self.add(c.arguments.get().len(), false),
                Block::Reasoning(r) => {
                    for part in r.summary.iter().chain(&r.text) {
                        self.add(part.len(), true);
                    }
                }
                _ => {}
            }
        }
    }

    fn add(&mut self, len: usize, reasoning: bool) {
        let len = u64::try_from(len).unwrap_or(u64::MAX);
        self.deltas = self.deltas.saturating_add(1);
        self.bytes = self.bytes.saturating_add(len);
        if reasoning {
            self.reasoning_deltas = self.reasoning_deltas.saturating_add(1);
            self.reasoning_bytes = self.reasoning_bytes.saturating_add(len);
        }
    }

    pub fn has_final_usage(&self) -> bool {
        self.final_usage
    }

    pub fn output_tokens(&self) -> u64 {
        self.deltas.max(tokens_of_bytes(self.bytes))
    }

    pub fn reasoning_tokens(&self) -> u64 {
        self.reasoning_deltas
            .max(tokens_of_bytes(self.reasoning_bytes))
    }
}

pub fn complete(reported: &Usage, tally: &Tally, input: Option<u64>) -> Usage {
    let mut usage = reported.clone();
    if usage.total_input_tokens() == 0
        && let Some(n) = input
    {
        usage.input_tokens = n;
    }
    usage.output_tokens = usage.output_tokens.max(tally.output_tokens());
    let reasoning = tally.reasoning_tokens();
    if reasoning > 0 {
        usage.reasoning_tokens = Some(usage.reasoning_tokens.unwrap_or(0).max(reasoning));
    }
    usage
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{BlockDelta, ResponseDelta, SummaryDelta};

    fn delta(index: u32, d: Delta) -> StreamEvent {
        StreamEvent::BlockDelta(BlockDelta::new(index, d))
    }

    #[test]
    fn deltas_count_by_number_and_by_size() {
        let mut t = Tally::new();
        for word in ["Hel", "lo", "!", " How", " are", " you"] {
            t.feed(&delta(0, Delta::Text(word.into())));
        }
        assert_eq!(t.output_tokens(), 6);
        let mut t = Tally::new();
        t.feed(&delta(0, Delta::Text("Here is a long story about ".into())));
        t.feed(&delta(
            1,
            Delta::ToolArguments("{\"city\": \"Paris\"}".into()),
        ));
        assert_eq!(t.output_tokens(), (27 + 17_u64).div_ceil(4));
        assert_eq!(t.reasoning_tokens(), 0);
        assert!(!t.has_final_usage());
    }

    #[test]
    fn reasoning_is_part_of_output_and_counted_apart() {
        let mut t = Tally::new();
        t.feed(&delta(
            0,
            Delta::ReasoningText("Let me think about this.".into()),
        ));
        let mut s = SummaryDelta::default();
        s.text = "Thinking".into();
        t.feed(&delta(1, Delta::ReasoningSummary(s)));
        t.feed(&delta(2, Delta::Text("Yes.".into())));
        assert_eq!(t.reasoning_tokens(), (23 + 8_u64).div_ceil(4));
        assert_eq!(t.output_tokens(), (23 + 8 + 4_u64).div_ceil(4));
    }

    #[test]
    fn final_usage_means_nothing_to_estimate() {
        let mut t = Tally::new();
        t.feed(&StreamEvent::ResponseDelta(ResponseDelta::default()));
        assert!(!t.has_final_usage());
        let mut d = ResponseDelta::default();
        d.usage = Some(Usage::default());
        t.feed(&StreamEvent::ResponseDelta(d));
        assert!(t.has_final_usage());
    }

    #[test]
    fn reported_usage_is_kept_and_completed() {
        let mut t = Tally::new();
        for _ in 0..30 {
            t.feed(&delta(0, Delta::Text("tok".into())));
        }
        let mut started = Usage::default();
        started.input_tokens = 10;
        started.cache_read_tokens = 5;
        started.output_tokens = 1;
        let u = complete(&started, &t, Some(999));
        assert_eq!((u.input_tokens, u.cache_read_tokens), (10, 5));
        assert_eq!(u.output_tokens, 30);
        assert_eq!(u.reasoning_tokens, None);
        let u = complete(&Usage::default(), &t, Some(42));
        assert_eq!((u.input_tokens, u.output_tokens), (42, 30));
        let mut more = started.clone();
        more.output_tokens = 100;
        assert_eq!(complete(&more, &t, None).output_tokens, 100);
    }

    #[test]
    fn bytes_round_up_to_tokens() {
        assert_eq!(tokens_of_bytes(0), 0);
        assert_eq!(tokens_of_bytes(1), 1);
        assert_eq!(tokens_of_bytes(8), 2);
        assert_eq!(tokens_of_bytes(9), 3);
    }
    #[test]
    fn a_whole_reply_counts_like_its_stream() {
        use kaeso_kpp::{RawJson, ReasoningBlock, RefusalBlock, TextBlock, ToolCallBlock};
        let mut reasoning = ReasoningBlock::default();
        reasoning.summary.push("Thinking".into());
        reasoning.text.push("Let me think about this.".into());
        let blocks = [
            Block::Reasoning(reasoning),
            Block::Text(TextBlock::new("Here is a long story about ")),
            Block::ToolCall(ToolCallBlock::new(
                "call_1",
                "get_weather",
                RawJson::from_string("{\"city\": \"Paris\"}".to_owned()).unwrap(),
            )),
            Block::Refusal(RefusalBlock::new("No.")),
        ];
        let mut t = Tally::new();
        t.feed_blocks(&blocks);
        assert_eq!(t.reasoning_tokens(), (8 + 24_u64).div_ceil(4));
        assert_eq!(t.output_tokens(), (8 + 24 + 27 + 17 + 3_u64).div_ceil(4));
        assert!(!t.has_final_usage());
    }
}
