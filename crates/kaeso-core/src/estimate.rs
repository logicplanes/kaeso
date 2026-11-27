use kaeso_kpp::{
    Block, BlockDelta, BlockStop, Delta, GenerateRequest, OutputSpec, ResponseDelta,
    ResponseStop, Source, StopReason, StreamEvent, ToolDef, Usage,
};

pub const BYTES_PER_TOKEN: u64 = 4;

pub fn tokens_of_bytes(bytes: u64) -> u64 {
    bytes.div_ceil(BYTES_PER_TOKEN)
}

/// What an image, and each page of a document, counts in [`input_tokens`]: about what Claude
/// counts for an image of 1.15 megapixels, which is more than other providers count for most
/// images (https://platform.claude.com/docs/en/build-with-claude/vision#calculate-image-costs).
pub const MEDIA_TOKENS: u64 = 1_600;

// The tokens a tokenizer writes around each message and each tool: its role, separators and
// the tool's framing.
const FRAMING_TOKENS: u64 = 4;

/// Kaeso's estimate of a request's input tokens, for a model whose provider can't count them.
/// Text counts a token for every four letters or digits of a word, rounded up, and one for each
/// punctuation mark, symbol, line break and character outside ASCII: within 0.7 to 1.05 of
/// Claude's own counts for English and German prose, Chinese, code and JSON, and above the
/// counts of tokenizers that pack text tighter. Tool definitions, tool calls and schemas count
/// as their JSON text. Each image and
/// each page of a PDF counts [`MEDIA_TOKENS`], as does a document Kaeso can't read the pages of.
/// Each message and each tool adds a few tokens of framing. `None` when the request holds audio
/// or video, whose length Kaeso doesn't read.
pub fn input_tokens(g: &GenerateRequest) -> Option<u64> {
    let mut n = blocks_tokens(&g.system)?;
    for m in &g.messages {
        n = n
            .saturating_add(FRAMING_TOKENS)
            .saturating_add(blocks_tokens(&m.blocks)?);
    }
    for tool in &g.tools {
        let t = match tool {
            ToolDef::Function(f) => text_tokens(&f.name)
                .saturating_add(f.description.as_deref().map_or(0, text_tokens))
                .saturating_add(f.parameters.as_ref().map_or(0, |p| text_tokens(p.get()))),
            ToolDef::Unknown(raw) => text_tokens(raw.raw.get()),
            _ => 0,
        };
        n = n.saturating_add(FRAMING_TOKENS).saturating_add(t);
    }
    if let OutputSpec::JsonSchema(spec) = &g.output {
        n = n.saturating_add(text_tokens(spec.schema.get()));
    }
    Some(n)
}

fn blocks_tokens(blocks: &[Block]) -> Option<u64> {
    blocks
        .iter()
        .try_fold(0u64, |n, b| Some(n.saturating_add(block_tokens(b)?)))
}

fn block_tokens(block: &Block) -> Option<u64> {
    Some(match block {
        Block::Text(t) => text_tokens(&t.text),
        Block::Refusal(r) => text_tokens(&r.text),
        Block::Image(_) => MEDIA_TOKENS,
        Block::Document(d) => {
            let title = d.title.as_deref().map_or(0, text_tokens);
            title.saturating_add(match &d.source {
                Source::Text(t) => text_tokens(&t.data),
                Source::Base64(b) if b.media_type.starts_with("text/") => {
                    text_tokens(&String::from_utf8_lossy(&b.data))
                }
                Source::Base64(b) => pdf_pages(&b.data).saturating_mul(MEDIA_TOKENS),
                _ => MEDIA_TOKENS,
            })
        }
        Block::ToolCall(c) => text_tokens(&c.name).saturating_add(text_tokens(c.arguments.get())),
        Block::ToolResult(r) => blocks_tokens(&r.content)?,
        Block::Reasoning(r) => r
            .summary
            .iter()
            .chain(&r.text)
            .fold(0u64, |n, s| n.saturating_add(text_tokens(s))),
        Block::Unknown(raw) => text_tokens(raw.raw.get()),
        Block::Json(j) => text_tokens(j.data.get()),
        Block::Audio(_) | Block::Video(_) => return None,
        _ => return None,
    })
}

fn text_tokens(text: &str) -> u64 {
    let (mut n, mut word) = (0u64, 0u64);
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            word = word.saturating_add(1);
            continue;
        }
        n = n.saturating_add(word.div_ceil(4));
        word = 0;
        if c == '\n' || !c.is_whitespace() {
            n = n.saturating_add(1);
        }
    }
    n.saturating_add(word.div_ceil(4))
}

// A PDF's pages, by its page objects (`/Type /Page`, not `/Pages`); at least one, so a file
// that isn't a PDF counts as one page.
fn pdf_pages(data: &[u8]) -> u64 {
    let mut pages = 0u64;
    let mut rest = data;
    while let Some(at) = find(rest, b"/Type") {
        let after = rest.get(at.saturating_add(5)..).unwrap_or_default();
        let name = after.trim_ascii_start();
        if name.starts_with(b"/Page") && !name.get(5..).unwrap_or_default().starts_with(b"s") {
            pages = pages.saturating_add(1);
        }
        rest = after;
    }
    pages.max(1)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
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

    pub fn visible_tokens(&self) -> u64 {
        self.deltas
            .saturating_sub(self.reasoning_deltas)
            .max(tokens_of_bytes(
                self.bytes.saturating_sub(self.reasoning_bytes),
            ))
    }
}

// For a route that can't take an output limit or stop sequences (a ChatGPT plan refuses the
// limit, OpenAI Responses has no `stop` and no limit under 16), Kaeso ends the reply the way a
// provider would: open blocks stop and the reply stops with `MaxTokens` or `StopSequence`.
#[derive(Clone, Debug)]
pub struct OutputCap {
    limit: Option<u64>,
    visible_only: bool,
    stop: Vec<String>,
    held: Vec<(u32, String)>,
    tally: Tally,
    open: Vec<u32>,
    ended: bool,
    reached: bool,
}

enum Scan {
    Pass(String),
    Stop(String, String),
}

impl OutputCap {
    pub fn new(limit: u64) -> OutputCap {
        OutputCap {
            limit: Some(limit),
            ..OutputCap::at_stop_sequences(Vec::new())
        }
    }

    pub fn at_stop_sequences(stop: Vec<String>) -> OutputCap {
        OutputCap {
            limit: None,
            visible_only: false,
            stop: stop.into_iter().filter(|s| !s.is_empty()).collect(),
            held: Vec::new(),
            tally: Tally::new(),
            open: Vec::new(),
            ended: false,
            reached: false,
        }
    }

    #[must_use]
    pub fn with_stop_sequences(mut self, stop: Vec<String>) -> OutputCap {
        self.stop = stop.into_iter().filter(|s| !s.is_empty()).collect();
        self
    }

    // For a provider whose output limit leaves out reasoning, as xAI's does.
    #[must_use]
    pub fn of_visible_output(mut self) -> OutputCap {
        self.visible_only = true;
        self
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
                StreamEvent::BlockStop(s) => {
                    self.release(Some(s.index), &mut out);
                    self.open.retain(|i| *i != s.index);
                }
                StreamEvent::ResponseDelta(d) if d.stop.is_some() => {
                    self.release(None, &mut out);
                    self.ended = true;
                }
                StreamEvent::ResponseStop(_) | StreamEvent::Error(_) => {
                    self.release(None, &mut out);
                    self.ended = true;
                }
                _ => {}
            }
            let event = match event {
                StreamEvent::BlockDelta(mut d) if !self.ended && !self.stop.is_empty() => {
                    let Delta::Text(text) = &d.delta else {
                        self.push(StreamEvent::BlockDelta(d), &mut out);
                        if self.over_limit() {
                            self.end(StopReason::MaxTokens, &mut out);
                            break;
                        }
                        continue;
                    };
                    match self.scan(d.index, text) {
                        Scan::Pass(text) if text.is_empty() => continue,
                        Scan::Pass(text) => {
                            d.delta = Delta::Text(text);
                            StreamEvent::BlockDelta(d)
                        }
                        Scan::Stop(text, sequence) => {
                            if !text.is_empty() {
                                d.delta = Delta::Text(text);
                                self.push(StreamEvent::BlockDelta(d), &mut out);
                            }
                            self.end(StopReason::StopSequence(sequence), &mut out);
                            break;
                        }
                    }
                }
                other => other,
            };
            let output = matches!(event, StreamEvent::BlockDelta(_));
            self.push(event, &mut out);
            if output && self.over_limit() {
                self.end(StopReason::MaxTokens, &mut out);
                break;
            }
        }
        out
    }

    fn push(&mut self, event: StreamEvent, out: &mut Vec<StreamEvent>) {
        self.tally.feed(&event);
        out.push(event);
    }

    fn over_limit(&self) -> bool {
        let used = if self.visible_only {
            self.tally.visible_tokens()
        } else {
            self.tally.output_tokens()
        };
        !self.ended && self.limit.is_some_and(|limit| used >= limit)
    }

    fn end(&mut self, reason: StopReason, out: &mut Vec<StreamEvent>) {
        self.reached = true;
        out.extend(
            self.open
                .drain(..)
                .map(|i| StreamEvent::BlockStop(BlockStop::new(i))),
        );
        out.push(StreamEvent::ResponseDelta(ResponseDelta::new(
            Some(reason),
            None,
        )));
        out.push(StreamEvent::ResponseStop(ResponseStop::new()));
    }

    // Text that could begin a stop sequence waits for the next delta of its block; it goes out
    // when the block or the reply ends without the sequence completing.
    fn release(&mut self, index: Option<u32>, out: &mut Vec<StreamEvent>) {
        let (released, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|(i, _)| index.is_none_or(|index| *i == index));
        self.held = kept;
        for (i, text) in released {
            self.push(
                StreamEvent::BlockDelta(BlockDelta::new(i, Delta::Text(text))),
                out,
            );
        }
    }

    fn scan(&mut self, index: u32, text: &str) -> Scan {
        let mut buf = match self.held.iter().position(|(i, _)| *i == index) {
            Some(at) => self.held.swap_remove(at).1,
            None => String::new(),
        };
        buf.push_str(text);
        let first = self
            .stop
            .iter()
            .filter_map(|s| buf.find(s.as_str()).map(|at| (at, s)))
            .min_by_key(|(at, _)| *at);
        if let Some((at, sequence)) = first {
            let sequence = sequence.clone();
            buf.truncate(at);
            return Scan::Stop(buf, sequence);
        }
        let keep = self.stop.iter().map(|s| begun(&buf, s)).max().unwrap_or(0);
        let at = buf.len().saturating_sub(keep);
        if keep > 0 && buf.is_char_boundary(at) {
            let held = buf.split_off(at);
            self.held.push((index, held));
        }
        Scan::Pass(buf)
    }
}

// The length of the longest start of `stop`, shorter than `stop`, that `text` ends with.
fn begun(text: &str, stop: &str) -> usize {
    (1..stop.len())
        .rev()
        .find(|&k| stop.get(..k).is_some_and(|start| text.ends_with(start)))
        .unwrap_or(0)
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

    fn texts(pieces: &[&str]) -> Vec<StreamEvent> {
        let mut events = vec![start(0, kaeso_kpp::BlockHeader::Text)];
        events.extend(pieces.iter().map(|t| delta(0, Delta::Text((*t).into()))));
        events
    }

    fn text_of(events: &[StreamEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::BlockDelta(d) => match &d.delta {
                    Delta::Text(t) => Some(t.as_str()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    fn ending(reason: StopReason) -> Vec<StreamEvent> {
        vec![
            StreamEvent::BlockStop(BlockStop::new(0)),
            StreamEvent::ResponseDelta(ResponseDelta::new(Some(reason), None)),
            StreamEvent::ResponseStop(ResponseStop::new()),
        ]
    }

    #[test]
    fn a_reply_ends_before_its_first_stop_sequence_even_when_deltas_split_it() {
        let mut cap = OutputCap::at_stop_sequences(vec!["Observation:".into(), "END".into()]);
        let out = cap.apply(texts(&[
            "Thought: look it up\nObs",
            "ervation: 42 END",
            " more",
        ]));
        assert!(cap.reached());
        assert_eq!(text_of(&out), "Thought: look it up\n");
        assert_eq!(
            out[out.len() - 3..],
            ending(StopReason::StopSequence("Observation:".into()))
        );
        assert!(
            cap.apply(texts(&["later"])).is_empty(),
            "nothing follows the cut"
        );
    }

    #[test]
    fn text_held_for_a_stop_sequence_goes_out_when_the_sequence_does_not_complete() {
        let mut cap = OutputCap::at_stop_sequences(vec!["END".into(), String::new()]);
        let mut events = texts(&["The E", "nd.", " Half E"]);
        events.extend(ending(StopReason::EndTurn));
        let out = cap.apply(events);
        assert!(!cap.reached(), "an empty sequence never matches");
        assert_eq!(text_of(&out), "The End. Half E");
        assert_eq!(
            out[out.len() - 4..],
            [
                vec![delta(0, Delta::Text("E".into()))],
                ending(StopReason::EndTurn)
            ]
            .concat(),
            "the held text goes out before its block stops"
        );
    }

    #[test]
    fn stop_sequences_end_text_only_and_share_the_reply_with_a_limit() {
        use kaeso_kpp::{BlockHeader, ToolCallHeader};
        let mut cap = OutputCap::new(3).with_stop_sequences(vec!["}".into()]);
        let out = cap.apply(vec![
            start(
                0,
                BlockHeader::ToolCall(ToolCallHeader::new("call_1", "search")),
            ),
            delta(0, Delta::ToolArguments("{}".into())),
            delta(0, Delta::ToolArguments("{}".into())),
            delta(0, Delta::ToolArguments("{}".into())),
            delta(0, Delta::ToolArguments("{}".into())),
        ]);
        assert!(cap.reached());
        assert_eq!(
            out.len(),
            1 + 3 + 3,
            "a `}}` in arguments doesn't stop them"
        );
        assert_eq!(out[out.len() - 3..], ending(StopReason::MaxTokens));
    }

    #[test]
    fn stop_sequences_match_whole_characters() {
        let mut cap = OutputCap::at_stop_sequences(vec!["。".into(), "終わり".into()]);
        let out = cap.apply(texts(&["你好", "。再见"]));
        assert_eq!(text_of(&out), "你好");
        assert_eq!(
            out[out.len() - 3..],
            ending(StopReason::StopSequence("。".into()))
        );
        let mut cap = OutputCap::at_stop_sequences(vec!["終わり".into()]);
        let mut events = texts(&["これで終", "わりです"]);
        events.extend(ending(StopReason::EndTurn));
        let out = cap.apply(events);
        assert_eq!(text_of(&out), "これで");
        assert_eq!(begun("これで終", "終わり"), "終".len());
        assert_eq!(begun("abc", "終わり"), 0);

    fn user(blocks: Vec<Block>) -> kaeso_kpp::GenerateRequest {
        kaeso_kpp::GenerateRequest::new("m")
            .with_message(kaeso_kpp::Message::new(kaeso_kpp::Role::User, blocks))
    }

    #[test]
    fn a_request_is_estimated_from_its_text_media_and_tools() {
        let text = user(vec![Block::text("Hello, world!")]);
        assert_eq!(
            input_tokens(&text),
            Some(6 + 4),
            "two words, two marks and a message"
        );
        let chinese = user(vec![Block::text("你好，世界")]);
        assert_eq!(input_tokens(&chinese), Some(5 + 4), "a token per character");

        let mut tools = user(vec![Block::text("Hi")]);
        let mut f = kaeso_kpp::FunctionTool::new(
            "get_weather",
            Some(kaeso_kpp::RawJson::from_string(r#"{"type":"object"}"#.to_owned()).unwrap()),
        );
        f.description = Some("Weather".into());
        tools.tools.push(ToolDef::Function(f));
        // "Hi" 1 + message 4, then the tool: name 4 (get, _, weather) + description 2 + schema 10
        // + framing 4.
        assert_eq!(input_tokens(&tools), Some(5 + 20));

        let image = user(vec![Block::image(Source::Url(
            "https://example.com/a.png".into(),
        ))]);
        assert_eq!(input_tokens(&image), Some(MEDIA_TOKENS + 4));
        let pdf = b"%PDF-1.7 1 0 obj << /Type /Pages /Kids [2 0 R 3 0 R 4 0 R] >> \
            2 0 obj << /Type /Page >> 3 0 obj <</Type/Page>> 4 0 obj << /Type  /Page /Parent 1 0 R >>";
        let doc = user(vec![Block::Document(kaeso_kpp::DocumentBlock::new(
            Source::bytes("application/pdf", pdf.to_vec()),
        ))]);
        assert_eq!(
            input_tokens(&doc),
            Some(3 * MEDIA_TOKENS + 4),
            "three pages"
        );
        let plain = user(vec![Block::Document(kaeso_kpp::DocumentBlock::new(
            Source::text("text/plain", "abcdefgh"),
        ))]);
        assert_eq!(input_tokens(&plain), Some(2 + 4));

        let audio = user(vec![Block::audio(Source::Url(
            "https://example.com/a.mp3".into(),
        ))]);
        assert_eq!(
            input_tokens(&audio),
            None,
            "audio has no length Kaeso reads"
        );
        let mut nested = user(vec![Block::tool_result(
            "call_1",
            vec![Block::audio(Source::Url(
                "https://example.com/a.mp3".into(),
            ))],
            false,
        )]);
        assert_eq!(input_tokens(&nested), None);
        nested.messages[0].blocks = vec![Block::tool_result(
            "call_1",
            vec![Block::text("12345678")],
            false,
        )];
        assert_eq!(input_tokens(&nested), Some(2 + 4));
    }
}
