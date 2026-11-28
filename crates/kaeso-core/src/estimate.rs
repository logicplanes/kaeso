use kaeso_kpp::{
    Block, BlockDelta, BlockStop, Delta, GenerateRequest, OutputSpec, ResponseDelta, ResponseStop,
    Source, StopReason, StreamEvent, ToolDef, Usage,
};

pub const BYTES_PER_TOKEN: u64 = 4;

pub fn tokens_of_bytes(bytes: u64) -> u64 {
    bytes.div_ceil(BYTES_PER_TOKEN)
}

// About what Claude counts for an image of 1.15 megapixels, more than other providers count for
// most: https://platform.claude.com/docs/en/build-with-claude/vision#calculate-image-costs
pub const MEDIA_TOKENS: u64 = 1_600;

// The tokens a tokenizer writes around each message and each tool: its role, separators and
// the tool's framing.
const FRAMING_TOKENS: u64 = 4;

// A token per four letters of a word and one per mark, line break or non-ASCII character came to
// 0.7 to 1.05 of Claude's own counts for prose, Chinese, code and JSON; 4 bytes a token, 0.52 to 0.75.
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

pub fn estimable(g: &GenerateRequest) -> bool {
    g.system
        .iter()
        .chain(g.messages.iter().flat_map(|m| &m.blocks))
        .all(countable)
}

fn countable(block: &Block) -> bool {
    match block {
        Block::Text(_)
        | Block::Refusal(_)
        | Block::Image(_)
        | Block::Document(_)
        | Block::ToolCall(_)
        | Block::Reasoning(_)
        | Block::Unknown(_)
        | Block::Json(_) => true,
        Block::ToolResult(r) => r.content.iter().all(countable),
        _ => false,
    }
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

// Page objects, each once whatever revisions incremental saves left of it, or the page tree's
// `/Count`, whichever is more: object streams may hide either. An outline's `/Count` is bookmarks.
fn pdf_pages(data: &[u8]) -> u64 {
    let (pages, tree) = page_counts(data);
    let found = pages.max(tree);
    if found <= 1 && find(data, b"/ObjStm").is_some() {
        return u64::try_from(data.len().div_ceil(50_000))
            .unwrap_or(u64::MAX)
            .max(1);
    }
    found.max(1)
}

// The page objects (`/Type /Page`) and the largest `/Count` of a `/Type /Pages` dictionary, in
// one pass that follows dictionary nesting and skips strings, comments and stream data, so none
// of them can mislead it.
fn page_counts(data: &[u8]) -> (u64, u64) {
    const DEPTH: usize = 64;
    // Bounds the memory a crafted file can take; real files have far fewer pages.
    const DISTINCT: usize = 65_536;
    let mut seen: std::collections::HashSet<(u64, u64)> = std::collections::HashSet::new();
    let mut object: Option<(u64, u64)> = None;
    #[derive(Default)]
    struct Dict {
        pages: bool,
        count: Option<u64>,
    }
    let mut open: Vec<Dict> = Vec::new();
    let (mut depth, mut best, mut pages, mut i) = (0usize, 0u64, 0u64, 0usize);
    while let Some(rest) = data.get(i..).filter(|r| !r.is_empty()) {
        if let Some(end) = skipped(rest) {
            i = i.saturating_add(end);
        } else if rest.starts_with(b"<<") {
            depth = depth.saturating_add(1);
            if depth <= DEPTH {
                open.push(Dict::default());
            }
            i = i.saturating_add(2);
        } else if rest.starts_with(b">>") {
            if depth > 0
                && depth <= DEPTH
                && let Some(d) = open.pop()
                && d.pages
            {
                best = best.max(d.count.unwrap_or(0));
            }
            depth = depth.saturating_sub(1);
            let after = i.saturating_add(2);
            i = match stream_after(data, after) {
                Some(at) => data
                    .get(at..)
                    .and_then(|r| find(r, b"endstream"))
                    .map_or(data.len(), |end| at.saturating_add(end).saturating_add(9)),
                None => after,
            };
        } else if let Some(after) = rest.strip_prefix(b"/Type") {
            let name = after.trim_ascii_start();
            if name.starts_with(b"/Pages") {
                if depth <= DEPTH
                    && let Some(d) = open.last_mut()
                {
                    d.pages = true;
                }
            } else if name.starts_with(b"/Page")
                && !name
                    .get(5..)
                    .is_some_and(|n| n.first().is_some_and(u8::is_ascii_alphanumeric))
            {
                let another = match object {
                    Some(id) if seen.contains(&id) => false,
                    Some(id) if seen.len() < DISTINCT => seen.insert(id),
                    _ => true,
                };
                if another {
                    pages = pages.saturating_add(1);
                }
            }
            i = i.saturating_add(5);
        } else if let Some(after) = rest.strip_prefix(b"/Count") {
            let digits: Vec<u8> = after
                .trim_ascii_start()
                .iter()
                .copied()
                .take_while(u8::is_ascii_digit)
                .take(19)
                .collect();
            if let Some(n) = std::str::from_utf8(&digits)
                .ok()
                .and_then(|d| d.parse().ok())
                && depth <= DEPTH
                && let Some(d) = open.last_mut()
            {
                d.count = Some(n);
            }
            i = i.saturating_add(6);
        } else if rest.starts_with(b"endobj") {
            object = None;
            i = i.saturating_add(6);
        } else if rest.starts_with(b"obj")
            && rest
                .get(3)
                .is_none_or(|c| c.is_ascii_whitespace() || b"<[/(%".contains(c))
        {
            object = object_id(data, i);
            i = i.saturating_add(3);
        } else {
            i = i.saturating_add(1);
        }
    }
    (pages, best)
}

// The object number and generation of an `N G obj` header whose `obj` starts at `at`.
fn object_id(data: &[u8], at: usize) -> Option<(u64, u64)> {
    let gap = |d: &[u8]| -> Option<usize> {
        let kept = d.trim_ascii_end().len();
        (kept < d.len()).then_some(kept)
    };
    let before = data.get(..at)?;
    let (rest, generation) = number_at_end(before.get(..gap(before)?)?)?;
    let (rest, number) = number_at_end(rest.get(..gap(rest)?)?)?;
    rest.last()
        .is_none_or(|c| !c.is_ascii_alphanumeric())
        .then_some((number, generation))
}

fn number_at_end(data: &[u8]) -> Option<(&[u8], u64)> {
    let digits = data
        .iter()
        .rev()
        .take(20)
        .take_while(|c| c.is_ascii_digit())
        .count();
    let (rest, number) = data.split_at(data.len().checked_sub(digits)?);
    Some((rest, std::str::from_utf8(number).ok()?.parse().ok()?))
}

// The length of a literal string `(…)`, a hex string `<…>` or a comment `%…` at the start of
// `rest`; a string's `)` or `>`, or a comment's line end, may be missing at the end of the data.
fn skipped(rest: &[u8]) -> Option<usize> {
    match rest.first()? {
        b'(' => {
            let (mut depth, mut escaped) = (0usize, false);
            for (k, c) in rest.iter().enumerate() {
                match c {
                    _ if escaped => escaped = false,
                    b'\\' => escaped = true,
                    b'(' => depth = depth.saturating_add(1),
                    b')' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Some(k.saturating_add(1));
                        }
                    }
                    _ => {}
                }
            }
            Some(rest.len())
        }
        b'<' if rest.get(1) != Some(&b'<') => Some(
            rest.iter()
                .position(|c| *c == b'>')
                .map_or(rest.len(), |k| k.saturating_add(1)),
        ),
        b'%' => Some(
            rest.iter()
                .position(|c| *c == b'\n' || *c == b'\r')
                .unwrap_or(rest.len()),
        ),
        _ => None,
    }
}

// Where a stream's `stream` keyword starts, when one follows a dictionary's `>>`, past the
// whitespace and comments PDF allows between them, looking at most 1 KiB ahead.
fn stream_after(data: &[u8], from: usize) -> Option<usize> {
    let window = data.get(from..from.saturating_add(1024).min(data.len()))?;
    let mut j = 0usize;
    while let Some(c) = window.get(j) {
        match c {
            c if c.is_ascii_whitespace() => j = j.saturating_add(1),
            b'%' => {
                let line = window
                    .get(j..)?
                    .iter()
                    .position(|c| *c == b'\n' || *c == b'\r')?;
                j = j.saturating_add(line);
            }
            _ => break,
        }
    }
    let at = from.saturating_add(j);
    data.get(at..)?.starts_with(b"stream").then_some(at)
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

#[derive(PartialEq, Eq)]
enum Output {
    Fits,
    Full,
    Cut,
}

// The provider's usage can come in the same chunk as the cut; the reply keeps it, not the
// provider's stop reason.
fn keep_usage(
    out: &mut [StreamEvent],
    usage: Option<Usage>,
    rest: impl Iterator<Item = StreamEvent>,
) {
    let usage = usage.or_else(|| {
        rest.filter_map(|e| match e {
            StreamEvent::ResponseDelta(d) => d.usage,
            _ => None,
        })
        .last()
    });
    let ending = out.iter_mut().rev().find_map(|e| match e {
        StreamEvent::ResponseDelta(d) => Some(d),
        _ => None,
    });
    if let (Some(usage), Some(ending)) = (usage, ending) {
        ending.usage = Some(usage);
    }
}

fn counted_text(delta: &mut Delta, visible_only: bool) -> Option<&mut String> {
    match delta {
        Delta::Text(s) | Delta::ToolArguments(s) | Delta::Refusal(s) => Some(s),
        Delta::ReasoningText(s) if !visible_only => Some(s),
        _ => None,
    }
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

    // Replies differ by where Kaeso ends them, which the provider request no longer says.
    pub fn cache_key(&self) -> String {
        format!(
            "limit={:?};visible={};stop={:?}",
            self.limit, self.visible_only, self.stop
        )
    }

    pub fn apply(&mut self, events: Vec<StreamEvent>) -> Vec<StreamEvent> {
        if self.reached {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(events.len());
        let mut events = events.into_iter();
        while let Some(event) = events.next() {
            let usage = match &event {
                StreamEvent::ResponseDelta(d) => d.usage.clone(),
                _ => None,
            };
            let full = match &event {
                StreamEvent::BlockStart(s) => {
                    self.open.push(s.index);
                    false
                }
                StreamEvent::BlockStop(s) => self.release(Some(s.index), &mut out),
                StreamEvent::ResponseDelta(d) if d.stop.is_some() => self.release(None, &mut out),
                StreamEvent::ResponseStop(_) | StreamEvent::Error(_) => {
                    self.release(None, &mut out)
                }
                _ => false,
            };
            if full {
                self.end(StopReason::MaxTokens, &mut out);
                keep_usage(&mut out, usage, events);
                break;
            }
            match &event {
                StreamEvent::BlockStop(s) => self.open.retain(|i| *i != s.index),
                StreamEvent::ResponseDelta(d) if d.stop.is_some() => self.ended = true,
                StreamEvent::ResponseStop(_) | StreamEvent::Error(_) => self.ended = true,
                _ => {}
            }
            let StreamEvent::BlockDelta(mut d) = event else {
                self.push(event, &mut out);
                continue;
            };
            if self.ended {
                self.push(StreamEvent::BlockDelta(d), &mut out);
                continue;
            }
            if let Delta::Text(text) = &d.delta
                && !self.stop.is_empty()
            {
                match self.scan(d.index, text) {
                    Scan::Pass(text) if text.is_empty() => continue,
                    Scan::Pass(text) => d.delta = Delta::Text(text),
                    Scan::Stop(text, sequence) => {
                        d.delta = Delta::Text(text);
                        let reason = match self.output(d, &mut out) {
                            Output::Cut => StopReason::MaxTokens,
                            Output::Fits | Output::Full => StopReason::StopSequence(sequence),
                        };
                        self.end(reason, &mut out);
                        keep_usage(&mut out, usage, events);
                        break;
                    }
                }
            }
            if self.output(d, &mut out) != Output::Fits {
                self.end(StopReason::MaxTokens, &mut out);
                keep_usage(&mut out, usage, events);
                break;
            }
        }
        out
    }

    fn push(&mut self, event: StreamEvent, out: &mut Vec<StreamEvent>) {
        self.tally.feed(&event);
        out.push(event);
    }

    // A delta goes out only as far as the limit reaches, measured as `Tally` measures it.
    fn output(&mut self, mut d: BlockDelta, out: &mut Vec<StreamEvent>) -> Output {
        let room = self.room();
        let visible_only = self.visible_only;
        let cut = match (room, counted_text(&mut d.delta, visible_only)) {
            (Some(room), Some(text)) if text.len() > room => {
                let at = (0..=room).rev().find(|&i| text.is_char_boundary(i));
                text.truncate(at.unwrap_or(0));
                true
            }
            _ => false,
        };
        let empty = counted_text(&mut d.delta, visible_only).is_some_and(|t| t.is_empty());
        if !empty {
            self.push(StreamEvent::BlockDelta(d), out);
        }
        if cut {
            Output::Cut
        } else if self.over_limit() {
            Output::Full
        } else {
            Output::Fits
        }
    }

    fn room(&self) -> Option<usize> {
        let limit = self.limit.filter(|_| !self.ended)?;
        let used = if self.visible_only {
            self.tally.bytes.saturating_sub(self.tally.reasoning_bytes)
        } else {
            self.tally.bytes
        };
        let room = limit.saturating_mul(BYTES_PER_TOKEN).saturating_sub(used);
        Some(usize::try_from(room).unwrap_or(usize::MAX))
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
    // when the block or the reply ends without the sequence completing. True when it used up
    // the limit.
    fn release(&mut self, index: Option<u32>, out: &mut Vec<StreamEvent>) -> bool {
        let (released, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|(i, _)| index.is_none_or(|index| *i == index));
        self.held = kept;
        for (i, text) in released {
            let d = BlockDelta::new(i, Delta::Text(text));
            if self.output(d, out) != Output::Fits {
                return true;
            }
        }
        false
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
        assert_eq!(text_of(&out), "one ");
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
    fn a_delta_goes_out_only_as_far_as_the_limit_reaches() {
        let mut events = texts(&["Water boils at 100 degrees."]);
        events.extend(ending(StopReason::EndTurn));
        let (out, reached) = capped_run(1, events);
        assert!(reached);
        assert_eq!(text_of(&out), "Wate", "one token is four bytes");
        assert_eq!(out[out.len() - 3..], ending(StopReason::MaxTokens));

        let mut cap = OutputCap::new(1);
        let out = cap.apply(texts(&["日本語"]));
        assert_eq!(text_of(&out), "日", "a character is never split");
        let mut cap = OutputCap::new(1).of_visible_output();
        let out = cap.apply(vec![
            start(0, kaeso_kpp::BlockHeader::Text),
            delta(
                0,
                Delta::ReasoningText("Thinking about it at length.".into()),
            ),
            delta(0, Delta::Text("Yes, indeed.".into())),
        ]);
        assert_eq!(
            text_of(&out),
            "Yes,",
            "reasoning isn't cut when it doesn't count"
        );
    }

    #[test]
    fn the_limit_ends_a_reply_before_a_later_stop_sequence_and_checks_held_text() {
        let mut cap = OutputCap::new(1).with_stop_sequences(vec!["END".into()]);
        let out = cap.apply(texts(&["Water boils END"]));
        assert_eq!(text_of(&out), "Wate");
        assert_eq!(out[out.len() - 3..], ending(StopReason::MaxTokens));

        let mut cap = OutputCap::new(3).with_stop_sequences(vec!["ENDOFTEXT".into()]);
        let mut events = texts(&["abcdefgENDOFTEX"]);
        events.extend(ending(StopReason::EndTurn));
        let out = cap.apply(events);
        assert_eq!(
            text_of(&out),
            "abcdefgENDOF",
            "held text released at the end counts"
        );
        assert_eq!(out[out.len() - 3..], ending(StopReason::MaxTokens));

        let mut cap = OutputCap::new(10).with_stop_sequences(vec!["END".into()]);
        let out = cap.apply(texts(&["Short END"]));
        assert_eq!(
            out[out.len() - 3..],
            ending(StopReason::StopSequence("END".into()))
        );
    }

    #[test]
    fn usage_in_the_chunk_of_the_cut_stays_with_the_reply() {
        let usage = Usage::new(40, 7);
        let mut events = texts(&["Hello END more"]);
        events.push(StreamEvent::BlockStop(BlockStop::new(0)));
        events.push(StreamEvent::ResponseDelta(ResponseDelta::new(
            Some(StopReason::EndTurn),
            Some(usage.clone()),
        )));
        events.push(StreamEvent::ResponseStop(ResponseStop::new()));
        let mut cap = OutputCap::at_stop_sequences(vec!["END".into()]);
        let out = cap.apply(events.clone());
        assert_eq!(text_of(&out), "Hello ");
        assert_eq!(
            out[out.len() - 2],
            StreamEvent::ResponseDelta(ResponseDelta::new(
                Some(StopReason::StopSequence("END".into())),
                Some(usage.clone())
            )),
            "the provider's usage, not its stop reason"
        );

        let mut cap = OutputCap::new(3).with_stop_sequences(vec!["ENDOFTEXT".into()]);
        let mut held = texts(&["abcdefgENDOFTEX"]);
        held.push(StreamEvent::ResponseDelta(ResponseDelta::new(
            Some(StopReason::EndTurn),
            Some(usage.clone()),
        )));
        let out = cap.apply(held);
        assert_eq!(
            out[out.len() - 2],
            StreamEvent::ResponseDelta(ResponseDelta::new(
                Some(StopReason::MaxTokens),
                Some(usage)
            )),
            "also when releasing held text at the provider's end uses up the limit"
        );
    }

    #[test]
    fn the_cache_key_names_every_rule_that_ends_a_reply() {
        let keys = [
            OutputCap::new(5).cache_key(),
            OutputCap::new(6).cache_key(),
            OutputCap::new(5).of_visible_output().cache_key(),
            OutputCap::at_stop_sequences(vec![" help".into()]).cache_key(),
            OutputCap::at_stop_sequences(vec![" today".into()]).cache_key(),
        ];
        for (i, a) in keys.iter().enumerate() {
            for b in keys.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
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
    }

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
        let pdf = b"%PDF-1.7\n1 0 obj << /Type /Pages /Kids [2 0 R 3 0 R 4 0 R] >> \
            2 0 obj << /Type /Page >> 3 0 obj <</Type/Page>> 4 0 obj << /Type  /Page /Parent 1 0 R >>";
        let doc = user(vec![Block::Document(kaeso_kpp::DocumentBlock::new(
            Source::bytes("application/pdf", pdf.to_vec()),
        ))]);
        assert_eq!(
            input_tokens(&doc),
            Some(3 * MEDIA_TOKENS + 4),
            "three pages"
        );
        assert_eq!(
            pdf_pages(b"%PDF-1.7\n1 0 obj << /Count 12 /Type /Pages /Kids [] >>"),
            12
        );
        assert_eq!(
            pdf_pages(
                b"%PDF-1.7\n1 0 obj << /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >> \
                  2 0 obj << /Type /Outlines /First 5 0 R /Last 6 0 R /Count 40 >> \
                  7 0 obj << /Title (Intro) /Parent 2 0 R /Count 12 >>"
            ),
            2,
            "an outline's bookmarks are no pages"
        );
        assert_eq!(
            pdf_pages(
                b"%PDF-1.7\n1 0 obj << /Resources << /Font << /F1 5 0 R >> >> /Type /Pages \
                  /Kids [3 0 R] /Count 3 >> 9 0 obj << /Length 28 >> stream\n<< /Type /Pages /Count 99 >>\nendstream"
            ),
            3,
            "inline resources come before the count, and stream data is no dictionary"
        );
        assert_eq!(
            pdf_pages(
                b"%PDF-1.7\n1 0 obj << /Title (Live stream notes) >> \
                  2 0 obj << /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >> 5 0 obj << /Length 3 >> stream\nabc\nendstream"
            ),
            2,
            "a title's `stream` is text"
        );
        assert_eq!(
            pdf_pages(
                b"%PDF-1.7\n1 0 obj << /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >> \
                  9 0 obj << /Length 28 >>\n% the page tree's copy\n\n   stream\n<< /Type /Pages /Count 99 >>\nendstream"
            ),
            2,
            "comments and whitespace may come before `stream`"
        );
        assert_eq!(
            pdf_pages(
                b"%PDF-1.7\n1 0 obj << /Contents (a >>\n% note\nstream of text) /T <73747265616d> >> \
                  2 0 obj << /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>"
            ),
            2,
            "strings are text, whatever they spell"
        );
        assert_eq!(
            pdf_pages(
                b"%PDF-1.7\n% << /Type /Pages /Count 40 >> /Type /Page /Type /Page\n\
                  1 0 obj << /Type /Pages /Kids [2 0 R] /Count 1 >> 2 0 obj << /Type /Page >> \
                  3 0 obj << /Length 17 >> stream\n/Type /Page /Type\nendstream"
            ),
            1,
            "a comment or stream that spells page trees or pages counts nothing"
        );
        assert_eq!(
            pdf_pages(
                b"%PDF-1.7\n9 0 obj << /Type /ObjStm /N 2 /Length 0 >> stream\nendstream endobj \
                  4 0 obj << /Type /Pages /Kids [5 0 R] /Count 1 /Parent 1 0 R >> endobj \
                  5 0 obj << /Type /Page /Parent 4 0 R >> endobj 6 0 obj << /Type /Page >> endobj \
                  7 0 obj<</Type/Page>>endobj"
            ),
            3,
            "a visible branch of a page tree whose root is packed counts no fewer than its pages"
        );
        assert_eq!(object_id(b"12 0 obj", 5), Some((12, 0)));
        assert_eq!(object_id(b"x12 0 obj", 6), None, "a number stands alone");
        assert_eq!(object_id(b"0 obj", 2), None, "a header has two numbers");
        assert_eq!(
            pdf_pages(
                b"%PDF-1.7\n1 0 obj << /Type /Pages /Kids [2 0 R 3 0 R] /Count 2 >> \
                  2 0 obj << /Type /Page /Parent 1 0 R >> 3 0 obj << /Type /Page /Parent 1 0 R >> \
                  %%EOF\n2 0 obj << /Type /Page /Parent 1 0 R /Annots [9 0 R] >> \
                  3 0 obj << /Type /Page /Parent 1 0 R /Annots [9 0 R] >> %%EOF\n\
                  2 0 obj << /Type /Page /Parent 1 0 R /Annots [10 0 R] >> \
                  3 0 obj << /Type /Page /Parent 1 0 R /Annots [10 0 R] >> %%EOF\n"
            ),
            2,
            "incremental saves keep old copies of a page; the page tree counts each once"
        );
        let mut many = b"%PDF-1.7\n".to_vec();
        for _ in 0..200_000 {
            many.extend_from_slice(b"/Count 1 ");
        }
        assert_eq!(pdf_pages(&many), 1, "one pass, whatever the PDF holds");
        let mut packed = b"%PDF-1.7\n5 0 obj << /Type /ObjStm /N 40 >>".to_vec();
        packed.resize(120_000, b'x');
        assert_eq!(
            pdf_pages(&packed),
            3,
            "a page for every 50 kB of a packed file"
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
        assert!(!estimable(&audio));
        let mut nested = user(vec![Block::tool_result(
            "call_1",
            vec![Block::audio(Source::Url(
                "https://example.com/a.mp3".into(),
            ))],
            false,
        )]);
        assert_eq!(input_tokens(&nested), None);
        assert!(!estimable(&nested));
        nested.messages[0].blocks = vec![Block::tool_result(
            "call_1",
            vec![Block::text("12345678")],
            false,
        )];
        assert_eq!(input_tokens(&nested), Some(2 + 4));
        assert!(estimable(&nested) && estimable(&doc) && estimable(&plain));
    }
}
