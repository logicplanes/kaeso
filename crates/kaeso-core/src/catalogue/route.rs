use std::collections::BTreeSet;

use kaeso_codecs::HeaderList;
use kaeso_kpp::{DialectId, Request};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::id::{ApiKeyId, CatalogueModelId, ServiceAccountId, UserId};

pub const MAX_STEPS: usize = 32;
pub const MAX_DEPTH: usize = 12;
pub const MAX_RULES: usize = 10;
pub const MIN_BRANCHES: usize = 2;
pub const MAX_BRANCHES: usize = 8;
pub const MAX_IDS: usize = 100;
pub const MAX_TIMEOUT_SECONDS: u32 = 3600;
pub const MAX_INPUT_TOKENS: u64 = 10_000_000;
pub const MAX_HEADER_NAME: usize = 100;
pub const MAX_HEADER_VALUE: usize = 200;

// Headers that carry credentials: a route never reads them, so a route can't be used to
// probe a key by trying prefixes.
const SECRET_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "x-api-key",
    "x-goog-api-key",
    "api-key",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(
        description = "A step of an alias's route: call a model, branch on a condition, or\n\
    split requests by weight."
    )
)]
#[cfg_attr(feature = "openapi", schema(as = RouteStep))]
pub enum Step {
    #[doc = "Calls a catalogue model, and on failure its fallback."]
    Model(ModelStep),
    #[doc = "Takes `then` when the rules hold for the request, otherwise `else`."]
    Condition(ConditionStep),
    #[doc = "Sends each request down one branch, chosen by weight."]
    Split(SplitStep),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", schema(description = "Calls a catalogue model."))]
#[cfg_attr(feature = "openapi", schema(as = RouteModelStep))]
pub struct ModelStep {
    #[doc = "The catalogue model."]
    pub model: CatalogueModelId,
    #[doc = "The step that takes over when this one fails for a reason in\n\
        `fall_back_on`."]
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(no_recursion))]
    pub fallback: Option<Box<Step>>,
    #[doc = "The failures that move on to `fallback`. Defaults to `unavailable`,\n\
        `limited` and `unsupported`."]
    #[serde(default = "default_fall_back_on")]
    pub fall_back_on: BTreeSet<Failure>,
    #[doc = "Seconds to wait for the model's first byte, and for a reply that isn't\n\
        streamed, before the step fails as `unavailable`. Defaults to the model's\n\
        own timeout."]
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(minimum = 1, maximum = 3600))]
    pub timeout_seconds: Option<u32>,
}

pub fn default_fall_back_on() -> BTreeSet<Failure> {
    BTreeSet::from([Failure::Unavailable, Failure::Limited, Failure::Unsupported])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(
        description = "Why a step failed: `unavailable` (the provider can't be reached, times\n\
    out, is overloaded, answers 5xx, refuses the provider key or doesn't serve the\n\
    model), `limited` (a rate limit or quota of the provider, or a Kaeso limit or\n\
    budget for the model), `unsupported` (the model can't take something the\n\
    request uses), `rejected` (the provider refuses the request itself, such as a\n\
    context that is too long)."
    )
)]
#[cfg_attr(feature = "openapi", schema(as = RouteFailure))]
pub enum Failure {
    Unavailable,
    Limited,
    Unsupported,
    Rejected,
}

impl Failure {
    pub const ALL: [Failure; 4] = [
        Failure::Unavailable,
        Failure::Limited,
        Failure::Unsupported,
        Failure::Rejected,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Failure::Unavailable => "unavailable",
            Failure::Limited => "limited",
            Failure::Unsupported => "unsupported",
            Failure::Rejected => "rejected",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(description = "Branches on rules about the request.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteConditionStep))]
pub struct ConditionStep {
    #[doc = "`all`: every rule must hold; `any`: one is enough."]
    #[serde(rename = "match")]
    pub matches: Match,
    #[doc = "The rules, 1 to 10."]
    pub rules: Vec<Rule>,
    #[doc = "The step for requests the rules hold for."]
    #[cfg_attr(feature = "openapi", schema(no_recursion))]
    pub then: Box<Step>,
    #[doc = "The step for every other request."]
    #[serde(rename = "else")]
    #[cfg_attr(feature = "openapi", schema(no_recursion))]
    pub otherwise: Box<Step>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", schema(description = "How rules combine."))]
#[cfg_attr(feature = "openapi", schema(as = RouteMatch))]
pub enum Match {
    All,
    Any,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "field", rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(description = "A rule about the request, named by its `field`.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteRule))]
pub enum Rule {
    #[doc = "A request header, such as one the app sets to say which plan its user is on."]
    Header(HeaderRule),
    #[doc = "The API format the app sent the request in."]
    Format(FormatRule),
    #[doc = "Something the request uses, such as images or tools."]
    Feature(FeatureRule),
    #[doc = "The request's input tokens, estimated from its size at 4 bytes a token."]
    InputTokens(TokensRule),
    #[doc = "The API key that sent the request."]
    Key(KeyRule),
    #[doc = "The member whose key or Playground sent the request."]
    Member(MemberRule),
    #[doc = "The service account whose key sent the request."]
    ServiceAccount(ServiceAccountRule),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(description = "A rule on a request header.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteHeaderRule))]
pub struct HeaderRule {
    #[doc = "The header's name in lowercase, such as `x-plan`."]
    #[cfg_attr(feature = "openapi", schema(example = "x-plan"))]
    pub name: String,
    #[doc = "How the header is compared."]
    pub op: HeaderOp,
    #[doc = "The text to compare with, exactly as sent; none for `present` and\n\
        `absent`."]
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(example = "paid"))]
    pub value: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(description = "How a header is compared with the rule's value.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteHeaderOp))]
pub enum HeaderOp {
    Equals,
    NotEquals,
    Contains,
    Present,
    Absent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(description = "A rule on the app's API format.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteFormatRule))]
pub struct FormatRule {
    #[doc = "`in` or `not_in` the listed formats."]
    pub op: SetOp,
    #[doc = "The formats."]
    pub value: Vec<AppFormat>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", schema(description = "An API format apps send."))]
#[cfg_attr(feature = "openapi", schema(as = RouteFormat))]
pub enum AppFormat {
    OpenaiChat,
    OpenaiResponses,
    AnthropicMessages,
    OpenaiEmbeddings,
}

impl AppFormat {
    pub fn of(dialect: DialectId) -> Option<AppFormat> {
        match dialect {
            DialectId::OpenAiChat => Some(AppFormat::OpenaiChat),
            DialectId::OpenAiResponses => Some(AppFormat::OpenaiResponses),
            DialectId::AnthropicMessages => Some(AppFormat::AnthropicMessages),
            DialectId::OpenAiEmbeddings => Some(AppFormat::OpenaiEmbeddings),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(description = "Whether the request's value is in the list or not.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteSetOp))]
pub enum SetOp {
    In,
    NotIn,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(description = "A rule on what the request uses.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteFeatureRule))]
pub struct FeatureRule {
    #[doc = "`used` or `not_used`."]
    pub op: UseOp,
    #[doc = "The feature."]
    pub value: Feature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(description = "Whether the request uses the feature.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteUseOp))]
pub enum UseOp {
    Used,
    NotUsed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(
        description = "Something a request can use: images, PDFs, tools, reasoning, a JSON\n\
    schema for its output, or a streamed reply."
    )
)]
#[cfg_attr(feature = "openapi", schema(as = RouteFeature))]
pub enum Feature {
    Images,
    Pdfs,
    Tools,
    Reasoning,
    StructuredOutput,
    Streaming,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(description = "A rule on the request's estimated input tokens.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteTokensRule))]
pub struct TokensRule {
    #[doc = "`at_least` or `below` the value."]
    pub op: CmpOp,
    #[doc = "Tokens, 1 to 10,000,000."]
    #[cfg_attr(feature = "openapi", schema(minimum = 1, maximum = 10_000_000))]
    pub value: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(description = "A comparison with a number.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteCmpOp))]
pub enum CmpOp {
    AtLeast,
    Below,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", schema(description = "A rule on the API key."))]
#[cfg_attr(feature = "openapi", schema(as = RouteKeyRule))]
pub struct KeyRule {
    #[doc = "`in` or `not_in` the listed keys."]
    pub op: SetOp,
    #[doc = "The keys, 1 to 100."]
    pub value: Vec<ApiKeyId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", schema(description = "A rule on the member."))]
#[cfg_attr(feature = "openapi", schema(as = RouteMemberRule))]
pub struct MemberRule {
    #[doc = "`in` or `not_in` the listed members."]
    pub op: SetOp,
    #[doc = "The members' user ids, 1 to 100."]
    pub value: Vec<UserId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(description = "A rule on the service account.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteServiceAccountRule))]
pub struct ServiceAccountRule {
    #[doc = "`in` or `not_in` the listed service accounts."]
    pub op: SetOp,
    #[doc = "The service accounts, 1 to 100."]
    pub value: Vec<ServiceAccountId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "openapi",
    schema(description = "Sends each request down one of its branches, by weight.")
)]
#[cfg_attr(feature = "openapi", schema(as = RouteSplitStep))]
pub struct SplitStep {
    #[doc = "`request`: each request is drawn anew; `caller`: each key, member or\n\
        service account stays on one branch; `header`: each value of `header`\n\
        stays on one branch."]
    pub by: SplitBy,
    #[doc = "The header a split `by` `header` reads, such as an end user's id.\n\
        Requests without it are drawn anew."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(example = "x-user-id"))]
    pub header: Option<String>,
    #[doc = "The branches, 2 to 8, whose weights add up to 100."]
    pub branches: Vec<Branch>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", schema(description = "What a split draws by."))]
#[cfg_attr(feature = "openapi", schema(as = RouteSplitBy))]
pub enum SplitBy {
    Request,
    Caller,
    Header,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", schema(description = "A branch of a split."))]
#[cfg_attr(feature = "openapi", schema(as = RouteBranch))]
pub struct Branch {
    #[doc = "The share of requests in percent, 1 to 99."]
    #[cfg_attr(feature = "openapi", schema(minimum = 1, maximum = 99))]
    pub weight: u8,
    #[doc = "The step for these requests."]
    #[cfg_attr(feature = "openapi", schema(no_recursion))]
    pub step: Step,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct RouteError {
    pub param: String,
    pub message: String,
}

fn refuse(path: &str, field: &str, message: &str) -> RouteError {
    let param = if field.is_empty() {
        path.to_owned()
    } else {
        format!("{path}.{field}")
    };
    RouteError {
        param,
        message: message.to_owned(),
    }
}

pub fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_HEADER_NAME
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b"!#$%&'*+-.^_`|~".contains(&b)
        })
}

fn unique<T: Ord>(items: &[T]) -> bool {
    let mut seen = BTreeSet::new();
    items.iter().all(|i| seen.insert(i))
}

impl Step {
    pub fn model(model: CatalogueModelId) -> Step {
        Step::Model(ModelStep {
            model,
            fallback: None,
            fall_back_on: default_fall_back_on(),
            timeout_seconds: None,
        })
    }

    pub fn validate(&self, known: &dyn Fn(CatalogueModelId) -> bool) -> Result<(), RouteError> {
        let mut steps = 0;
        self.check("route", 1, &mut steps, known)
    }

    fn check(
        &self,
        path: &str,
        depth: usize,
        steps: &mut usize,
        known: &dyn Fn(CatalogueModelId) -> bool,
    ) -> Result<(), RouteError> {
        *steps += 1;
        if *steps > MAX_STEPS {
            return Err(refuse(path, "", "A route has at most 32 steps."));
        }
        if depth > MAX_DEPTH {
            return Err(refuse(path, "", "A route is at most 12 steps deep."));
        }
        match self {
            Step::Model(m) => {
                if !known(m.model) {
                    return Err(refuse(
                        path,
                        "model",
                        "This model is not in the project's catalogue.",
                    ));
                }
                if m.timeout_seconds
                    .is_some_and(|s| s == 0 || s > MAX_TIMEOUT_SECONDS)
                {
                    return Err(refuse(
                        path,
                        "timeout_seconds",
                        "A timeout is 1 to 3600 seconds.",
                    ));
                }
                if let Some(f) = &m.fallback {
                    if m.fall_back_on.is_empty() {
                        return Err(refuse(
                            path,
                            "fall_back_on",
                            "Choose when this step falls back, or remove its fallback.",
                        ));
                    }
                    f.check(&format!("{path}.fallback"), depth + 1, steps, known)?;
                }
                Ok(())
            }
            Step::Condition(c) => {
                if c.rules.is_empty() || c.rules.len() > MAX_RULES {
                    return Err(refuse(path, "rules", "A condition has 1 to 10 rules."));
                }
                for (i, r) in c.rules.iter().enumerate() {
                    r.check(&format!("{path}.rules.{i}"))?;
                }
                c.then
                    .check(&format!("{path}.then"), depth + 1, steps, known)?;
                c.otherwise
                    .check(&format!("{path}.else"), depth + 1, steps, known)
            }
            Step::Split(s) => {
                match (s.by, s.header.as_deref()) {
                    (SplitBy::Header, Some(name)) => header_name(path, "header", name)?,
                    (SplitBy::Header, None) => {
                        return Err(refuse(path, "header", "Name the header this split reads."));
                    }
                    (_, Some(_)) => {
                        return Err(refuse(
                            path,
                            "header",
                            "Only a split by header reads a header; remove `header`.",
                        ));
                    }
                    (_, None) => {}
                }
                if s.branches.len() < MIN_BRANCHES || s.branches.len() > MAX_BRANCHES {
                    return Err(refuse(path, "branches", "A split has 2 to 8 branches."));
                }
                for (i, b) in s.branches.iter().enumerate() {
                    if b.weight == 0 || b.weight > 99 {
                        return Err(refuse(
                            &format!("{path}.branches.{i}"),
                            "weight",
                            "A branch's weight is 1 to 99 percent.",
                        ));
                    }
                }
                let total: u32 = s.branches.iter().map(|b| u32::from(b.weight)).sum();
                if total != 100 {
                    return Err(refuse(
                        path,
                        "branches",
                        "A split's weights add up to 100 percent.",
                    ));
                }
                for (i, b) in s.branches.iter().enumerate() {
                    b.step.check(
                        &format!("{path}.branches.{i}.step"),
                        depth + 1,
                        steps,
                        known,
                    )?;
                }
                Ok(())
            }
        }
    }

    pub fn models(&self) -> BTreeSet<CatalogueModelId> {
        let mut out = BTreeSet::new();
        self.visit(&mut |m| {
            out.insert(m.model);
        });
        out
    }

    pub fn models_in_order(&self) -> Vec<CatalogueModelId> {
        let mut out = Vec::new();
        self.visit(&mut |m| {
            if !out.contains(&m.model) {
                out.push(m.model);
            }
        });
        out
    }

    pub fn uses(&self, model: CatalogueModelId) -> bool {
        let mut found = false;
        self.visit(&mut |m| found |= m.model == model);
        found
    }

    fn visit(&self, f: &mut dyn FnMut(&ModelStep)) {
        match self {
            Step::Model(m) => {
                f(m);
                if let Some(next) = &m.fallback {
                    next.visit(f);
                }
            }
            Step::Condition(c) => {
                c.then.visit(f);
                c.otherwise.visit(f);
            }
            Step::Split(s) => {
                for b in &s.branches {
                    b.step.visit(f);
                }
            }
        }
    }

    pub fn primary(&self) -> Option<CatalogueModelId> {
        match self {
            Step::Model(m) => Some(m.model),
            Step::Condition(c) => c.then.primary(),
            Step::Split(s) => s
                .branches
                .iter()
                .rev()
                .max_by_key(|b| b.weight)
                .and_then(|b| b.step.primary()),
        }
    }

    pub fn plan<'r>(&'r self, facts: &Facts<'_>) -> Vec<Planned<'r>> {
        self.trace(facts)
            .into_iter()
            .filter_map(|t| match t {
                Traced::Model { path, step } => Some(Planned { step, path }),
                Traced::Condition { .. } | Traced::Split { .. } => None,
            })
            .collect()
    }

    pub fn trace<'r>(&'r self, facts: &Facts<'_>) -> Vec<Traced<'r>> {
        let mut out = Vec::new();
        self.walk("route".to_owned(), facts, &mut out);
        out
    }

    fn walk<'r>(&'r self, path: String, facts: &Facts<'_>, out: &mut Vec<Traced<'r>>) {
        match self {
            Step::Model(m) => {
                let next = format!("{path}.fallback");
                out.push(Traced::Model { path, step: m });
                if let Some(fallback) = &m.fallback {
                    fallback.walk(next, facts, out);
                }
            }
            Step::Condition(c) => {
                let rules: Vec<bool> = c.rules.iter().map(|r| r.holds(facts)).collect();
                let holds = match c.matches {
                    Match::All => rules.iter().all(|r| *r),
                    Match::Any => rules.iter().any(|r| *r),
                };
                let (next, step) = if holds {
                    (format!("{path}.then"), &c.then)
                } else {
                    (format!("{path}.else"), &c.otherwise)
                };
                out.push(Traced::Condition { path, rules, holds });
                step.walk(next, facts, out);
            }
            Step::Split(s) => {
                let draw = s.draw(&path, facts);
                let branch = s.branch_for(draw);
                if let Some(b) = s.branches.get(branch) {
                    let next = format!("{path}.branches.{branch}.step");
                    out.push(Traced::Split {
                        path,
                        branch,
                        weight: b.weight,
                        draw,
                    });
                    b.step.walk(next, facts, out);
                }
            }
        }
    }
}

impl ConditionStep {
    pub fn holds(&self, facts: &Facts<'_>) -> bool {
        match self.matches {
            Match::All => self.rules.iter().all(|r| r.holds(facts)),
            Match::Any => self.rules.iter().any(|r| r.holds(facts)),
        }
    }
}

impl SplitStep {
    pub fn pick(&self, path: &str, facts: &Facts<'_>) -> usize {
        self.branch_for(self.draw(path, facts))
    }

    // The alias and the step's path make each split draw on its own; the subject keeps
    // a caller or a header value on one branch across requests.
    pub fn draw(&self, path: &str, facts: &Facts<'_>) -> u8 {
        let header = match (self.by, self.header.as_deref()) {
            (SplitBy::Header, Some(name)) => facts.headers.get_all(name).next(),
            _ => None,
        };
        let subject: &[u8] = match (self.by, &facts.caller, header) {
            (SplitBy::Caller, Some(caller), _) => caller.as_bytes(),
            (SplitBy::Header, _, Some(value)) => value.as_bytes(),
            _ => facts.request.as_bytes(),
        };
        let mut hash = Sha256::new();
        hash.update(facts.alias.as_bytes());
        hash.update(path.as_bytes());
        hash.update(subject);
        let digest = hash.finalize();
        let mut first = [0u8; 8];
        first.copy_from_slice(digest.get(..8).unwrap_or(&[0; 8]));
        u8::try_from(u64::from_be_bytes(first) % 100).unwrap_or(0)
    }

    pub fn branch_for(&self, draw: u8) -> usize {
        let mut upto = 0u32;
        for (i, b) in self.branches.iter().enumerate() {
            upto += u32::from(b.weight);
            if u32::from(draw) < upto {
                return i;
            }
        }
        self.branches.len().saturating_sub(1)
    }
}

impl Rule {
    fn check(&self, path: &str) -> Result<(), RouteError> {
        match self {
            Rule::Header(h) => {
                header_name(path, "name", &h.name)?;
                match (h.op, &h.value) {
                    (HeaderOp::Present | HeaderOp::Absent, None) => Ok(()),
                    (HeaderOp::Present | HeaderOp::Absent, Some(_)) => Err(refuse(
                        path,
                        "value",
                        "This rule only checks whether the header is sent; remove its value.",
                    )),
                    (_, Some(v))
                        if !v.is_empty()
                            && v.chars().count() <= MAX_HEADER_VALUE
                            && !v.chars().any(char::is_control) =>
                    {
                        Ok(())
                    }
                    _ => Err(refuse(
                        path,
                        "value",
                        "Give the header's value, up to 200 characters.",
                    )),
                }
            }
            Rule::Format(f) => {
                if f.value.is_empty() || !unique(&f.value) {
                    return Err(refuse(
                        path,
                        "value",
                        "Choose each format once, at least one.",
                    ));
                }
                Ok(())
            }
            Rule::Feature(_) => Ok(()),
            Rule::InputTokens(t) => {
                if t.value == 0 || t.value > MAX_INPUT_TOKENS {
                    return Err(refuse(path, "value", "Tokens are 1 to 10,000,000."));
                }
                Ok(())
            }
            Rule::Key(k) => ids(path, &k.value),
            Rule::Member(m) => ids(path, &m.value),
            Rule::ServiceAccount(s) => ids(path, &s.value),
        }
    }

    pub fn holds(&self, facts: &Facts<'_>) -> bool {
        match self {
            Rule::Header(h) => {
                let mut values = facts.headers.get_all(&h.name);
                match (h.op, h.value.as_deref()) {
                    (HeaderOp::Present, _) => values.next().is_some(),
                    (HeaderOp::Absent, _) => values.next().is_none(),
                    (HeaderOp::Equals, Some(v)) => values.any(|x| x == v),
                    (HeaderOp::NotEquals, Some(v)) => values.all(|x| x != v),
                    (HeaderOp::Contains, Some(v)) => values.any(|x| x.contains(v)),
                    (_, None) => false,
                }
            }
            Rule::Format(f) => member(f.op, facts.format.is_some_and(|x| f.value.contains(&x))),
            Rule::Feature(f) => {
                let used = facts.uses.contains(&f.value);
                match f.op {
                    UseOp::Used => used,
                    UseOp::NotUsed => !used,
                }
            }
            Rule::InputTokens(t) => match t.op {
                CmpOp::AtLeast => facts.input_tokens >= t.value,
                CmpOp::Below => facts.input_tokens < t.value,
            },
            Rule::Key(k) => member(k.op, facts.key.is_some_and(|id| k.value.contains(&id))),
            Rule::Member(m) => member(m.op, facts.member.is_some_and(|id| m.value.contains(&id))),
            Rule::ServiceAccount(s) => member(
                s.op,
                facts
                    .service_account
                    .is_some_and(|id| s.value.contains(&id)),
            ),
        }
    }
}

pub const MAX_TRACE_HEADERS: usize = 20;

pub fn check_trace_headers(headers: &[(String, String)]) -> Result<(), RouteError> {
    if headers.len() > MAX_TRACE_HEADERS {
        return Err(refuse("request", "headers", "Send up to 20 headers."));
    }
    for (i, (name, value)) in headers.iter().enumerate() {
        let path = format!("request.headers.{i}");
        header_name(&path, "name", name)?;
        if value.chars().count() > MAX_HEADER_VALUE || value.chars().any(char::is_control) {
            return Err(refuse(
                &path,
                "value",
                "A header's value is up to 200 characters.",
            ));
        }
    }
    Ok(())
}

fn header_name(path: &str, field: &str, name: &str) -> Result<(), RouteError> {
    if !valid_header_name(name) {
        return Err(refuse(
            path,
            field,
            "A header name is a lowercase HTTP token of up to 100 characters.",
        ));
    }
    if SECRET_HEADERS.contains(&name) {
        return Err(refuse(
            path,
            field,
            "Routes can't read this header, since it carries credentials.",
        ));
    }
    Ok(())
}

fn ids<T: Ord>(path: &str, list: &[T]) -> Result<(), RouteError> {
    if list.is_empty() || list.len() > MAX_IDS || !unique(list) {
        return Err(refuse(path, "value", "List 1 to 100 entries, each once."));
    }
    Ok(())
}

fn member(op: SetOp, found: bool) -> bool {
    match op {
        SetOp::In => found,
        SetOp::NotIn => !found,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Traced<'r> {
    Model {
        path: String,
        step: &'r ModelStep,
    },
    Condition {
        path: String,
        rules: Vec<bool>,
        holds: bool,
    },
    Split {
        path: String,
        branch: usize,
        weight: u8,
        draw: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned<'r> {
    pub step: &'r ModelStep,
    pub path: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Facts<'a> {
    pub alias: Uuid,
    pub request: Uuid,
    pub caller: Option<Uuid>,
    pub format: Option<AppFormat>,
    pub headers: &'a HeaderList,
    pub uses: &'a BTreeSet<Feature>,
    pub input_tokens: u64,
    pub key: Option<ApiKeyId>,
    pub member: Option<UserId>,
    pub service_account: Option<ServiceAccountId>,
}

pub fn features_of(request: &Request) -> BTreeSet<Feature> {
    let mut out = BTreeSet::new();
    if let Request::Generate(g) | Request::CountTokens(g) = request {
        let caps = g.required_capabilities();
        for (on, feature) in [
            (caps.images, Feature::Images),
            (caps.pdf, Feature::Pdfs),
            (caps.tools, Feature::Tools),
            (caps.reasoning, Feature::Reasoning),
            (caps.json_schema, Feature::StructuredOutput),
            (caps.streaming, Feature::Streaming),
        ] {
            if on {
                out.insert(feature);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
