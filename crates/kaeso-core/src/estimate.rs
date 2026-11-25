use kaeso_kpp::{
    Block, BlockStop, Delta, ResponseDelta, ResponseStop, StopReason, StreamEvent, Usage,
};

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

    pub fn from_counts(
        deltas: u64,
        bytes: u64,
        reasoning_deltas: u64,
        reasoning_bytes: u64,
        final_usage: bool,
    ) -> Tally {
        Tally {
            deltas,
            bytes,
            reasoning_deltas,
            reasoning_bytes,
            final_usage,
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

// For a route that refuses the output limit field (a ChatGPT plan), Kaeso ends the reply the
// way a provider would at the limit: open blocks stop and the reply stops with `MaxTokens`.
#[derive(Clone, Debug)]
pub struct OutputCap {
    limit: u64,
    tally: Tally,
    open: Vec<u32>,
    ended: bool,
    reached: bool,
}

impl OutputCap {
    pub fn new(limit: u64) -> OutputCap {
        OutputCap {
            limit,
            tally: Tally::new(),
            open: Vec::new(),
            ended: false,
            reached: false,
        }
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn reached(&self) -> bool {
        self.reached
    }

    pub fn apply(&mut self, events: Vec<StreamEvent>) -> Vec<StreamEvent> {
        if self.reached {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(events.len());
        for event in events {
            match &event {
                StreamEvent::BlockStart(s) => self.open.push(s.index),
                StreamEvent::BlockStop(s) => self.open.retain(|i| *i != s.index),
                StreamEvent::ResponseDelta(d) if d.stop.is_some() => self.ended = true,
                StreamEvent::ResponseStop(_) | StreamEvent::Error(_) => self.ended = true,
                _ => {}
            }
            let output = matches!(event, StreamEvent::BlockDelta(_));
            self.tally.feed(&event);
            out.push(event);
            if output && !self.ended && self.tally.output_tokens() >= self.limit {
                self.reached = true;
                out.extend(
                    self.open
                        .drain(..)
                        .map(|i| StreamEvent::BlockStop(BlockStop::new(i))),
                );
                out.push(StreamEvent::ResponseDelta(ResponseDelta::new(
                    Some(StopReason::MaxTokens),
                    None,
                )));
                out.push(StreamEvent::ResponseStop(ResponseStop::new()));
                break;
            }
        }
        out
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

    fn capped_run(limit: u64, events: Vec<StreamEvent>) -> (Vec<StreamEvent>, bool) {
        let mut cap = OutputCap::new(limit);
        let out = cap.apply(events);
        (out, cap.reached())
    }

    fn start(index: u32, block: kaeso_kpp::BlockHeader) -> StreamEvent {
        StreamEvent::BlockStart(kaeso_kpp::BlockStart::new(index, block))
    }

    #[test]
    fn a_reply_at_its_output_limit_stops_with_max_tokens_and_closes_its_blocks() {
        use kaeso_kpp::{BlockHeader, ResponseStart, ToolCallHeader};
        let mut events = vec![
            StreamEvent::ResponseStart(ResponseStart::new("resp_1", "m")),
            start(0, BlockHeader::Text),
            delta(0, Delta::Text("Calling.".into())),
            StreamEvent::BlockStop(BlockStop::new(0)),
            start(
                1,
                BlockHeader::ToolCall(ToolCallHeader::new("call_1", "search")),
            ),
        ];
        events.extend((0..10).map(|_| delta(1, Delta::ToolArguments("{\"q\":".into()))));
        let (out, reached) = capped_run(5, events);
        assert!(reached);
        let tail: Vec<_> = out.iter().rev().take(4).rev().cloned().collect();
        assert_eq!(
            tail,
            vec![
                delta(1, Delta::ToolArguments("{\"q\":".into())),
                StreamEvent::BlockStop(BlockStop::new(1)),
                StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::MaxTokens), None)),
                StreamEvent::ResponseStop(ResponseStop::new()),
            ]
        );
        assert_eq!(
            out.len(),
            5 + 2 + 3,
            "the deltas after the second call delta are dropped"
        );
    }

    #[test]
    fn nothing_follows_a_capped_reply_and_a_finished_reply_is_never_capped() {
        let mut cap = OutputCap::new(2);
        let first = cap.apply(vec![
            start(0, kaeso_kpp::BlockHeader::Text),
            delta(0, Delta::Text("a".into())),
            delta(0, Delta::Text("b".into())),
        ]);
        assert!(cap.reached());
        assert_eq!(first.len(), 6);
        assert!(
            cap.apply(vec![delta(0, Delta::Text("c".into()))])
                .is_empty()
        );

        let events = vec![
            start(0, kaeso_kpp::BlockHeader::Text),
            delta(0, Delta::Text("one two".into())),
            StreamEvent::BlockStop(BlockStop::new(0)),
            StreamEvent::ResponseDelta(ResponseDelta::new(Some(StopReason::EndTurn), None)),
            StreamEvent::ResponseStop(ResponseStop::new()),
        ];
        let (out, reached) = capped_run(1, events.clone());
        assert!(reached, "the delta that uses up the limit ends the reply");
        assert_eq!(out.len(), 2 + 3);
        let (out, reached) = capped_run(100, events.clone());
        assert!(!reached);
        assert_eq!(out, events);
    }
}
