use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use futures::future::BoxFuture;
use kaeso_core::cost::Unpriced;
use kaeso_core::limits::{self, Kind, Level, Limit, Period, Subject};
use kaeso_store::StoreError;
use kaeso_store::pg::limits as store;
use redis::Script;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::model_api::hooks::{Admission, Caller, Limits, Refusal, Reservation};
use crate::state::AppState;

mod flights;

pub type Rebuilds = flights::Flights<LimitsError>;

const IN_FLIGHT_MARGIN: Duration = Duration::from_secs(300);

const VALKEY_DEADLINE: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum LimitsError {
    #[error("valkey: {0}")]
    Valkey(#[from] redis::RedisError),
    #[error("valkey did not answer in time")]
    Timeout,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("unexpected limit script answer {0:?}")]
    Protocol(Vec<i64>),
    #[error("counters could not be rebuilt")]
    NotRebuilt,
}

fn in_flight_ttl(timeout: Duration) -> i64 {
    i64::try_from(timeout.saturating_add(IN_FLIGHT_MARGIN).as_secs()).unwrap_or(i64::MAX)
}

const ADMIT: &str = r"
local now = tonumber(ARGV[1])
local req = ARGV[2]
local ttl = tonumber(ARGV[#ARGV])
local missing = {}
local own = {}
local function flying(key)
  local f = key .. ':f'
  local r = key .. ':r'
  local gone = redis.call('ZRANGEBYSCORE', f, '-inf', now)
  for _, id in ipairs(gone) do redis.call('HDEL', r, id) end
  if #gone > 0 then redis.call('ZREMRANGEBYSCORE', f, '-inf', now) end
  local ids = redis.call('ZRANGE', f, 0, -1)
  local held = 0
  for _, id in ipairs(ids) do
    held = held + tonumber(redis.call('HGET', r, id) or '0')
  end
  return #ids, held
end
for i = 1, #KEYS do
  local b = 3 + (i - 1) * 5
  local t = tonumber(ARGV[b])
  local units = tonumber(ARGV[b + 1])
  local enforced = ARGV[b + 2] == '1'
  local reserve = tonumber(ARGV[b + 3])
  local bounded = ARGV[b + 4] == '1'
  local key = KEYS[i]
  if t == 1 or t == 2 then
    local fields = redis.call('HGETALL', key)
    local sum = 0
    local oldest = nil
    for j = 1, #fields, 2 do
      local sec = tonumber(fields[j])
      if sec <= now - 60 then
        redis.call('HDEL', key, fields[j])
      else
        sum = sum + tonumber(fields[j + 1])
        if oldest == nil or sec < oldest then oldest = sec end
      end
    end
    local retry = 1
    if oldest ~= nil then retry = math.max(oldest + 60 - now, 1) end
    if enforced and sum >= units then return {1, i, retry} end
    if t == 2 then
      if bounded and sum + reserve > units then
        if reserve > units then return {4, i} end
        return {1, i, retry}
      end
      local n, held = flying(key)
      if n > 0 and sum + held + reserve > units then return {3, i} end
      own[i] = reserve
    end
  else
    local v = redis.call('HGET', key, 'v')
    if not v then
      table.insert(missing, i)
    elseif enforced then
      v = tonumber(v)
      if v >= units then return {1, i, 0} end
      local count = tonumber(redis.call('HGET', key, 'n') or '0')
      local avg = tonumber(redis.call('HGET', key, 'a') or '0')
      if count > 0 then avg = math.ceil(v / count) end
      local mine = math.max(avg, reserve)
      if bounded and v + reserve > units then return {4, i} end
      local n, held = flying(key)
      if n > 0 then
        if not bounded and avg <= 0 then return {3, i} end
        if v + held + mine > units then return {3, i} end
      end
      own[i] = mine
    end
  end
end
if #missing > 0 then
  local out = {2}
  for _, i in ipairs(missing) do table.insert(out, i) end
  return out
end
for i = 1, #KEYS do
  local t = tonumber(ARGV[3 + (i - 1) * 5])
  if t == 1 then
    redis.call('HINCRBY', KEYS[i], tostring(now), 1)
    redis.call('EXPIRE', KEYS[i], 120)
  elseif own[i] ~= nil then
    local f = KEYS[i] .. ':f'
    local r = KEYS[i] .. ':r'
    redis.call('ZADD', f, now + ttl, req)
    redis.call('HSET', r, req, own[i])
    if redis.call('TTL', f) < ttl then redis.call('EXPIRE', f, ttl) end
    if redis.call('TTL', r) < ttl then redis.call('EXPIRE', r, ttl) end
  end
end
return {0}
";

const EXTEND: &str = r"
local score = tonumber(ARGV[2])
local ttl = tonumber(ARGV[3])
for i = 1, #KEYS do
  local f = KEYS[i] .. ':f'
  if redis.call('ZADD', f, 'XX', 'CH', score, ARGV[1]) == 1 then
    local r = KEYS[i] .. ':r'
    if redis.call('TTL', f) < ttl then redis.call('EXPIRE', f, ttl) end
    if redis.call('TTL', r) < ttl then redis.call('EXPIRE', r, ttl) end
  end
end
return 1
";

const CHARGE: &str = r"
local now = tonumber(ARGV[1])
local req = ARGV[2]
local succeeded = ARGV[3] == '1'
local out = {}
for i = 1, #KEYS do
  local b = 4 + (i - 1) * 3
  local t = tonumber(ARGV[b])
  local add = tonumber(ARGV[b + 1])
  local ttl = tonumber(ARGV[b + 2])
  local key = KEYS[i]
  if t == 2 then
    redis.call('ZREM', key .. ':f', req)
    redis.call('HDEL', key .. ':r', req)
    if add > 0 then
      redis.call('HINCRBY', key, tostring(now), add)
      redis.call('EXPIRE', key, 120)
    end
    table.insert(out, -1)
    table.insert(out, -1)
  elseif t == 3 then
    redis.call('ZREM', key .. ':f', req)
    redis.call('HDEL', key .. ':r', req)
    local v = redis.call('HGET', key, 'v')
    if v then
      local after = tonumber(v)
      if add > 0 then
        after = redis.call('HINCRBY', key, 'v', add)
      end
      if succeeded then redis.call('HINCRBY', key, 'n', 1) end
      if ttl > 0 then redis.call('EXPIRE', key, ttl) end
      table.insert(out, tonumber(v))
      table.insert(out, after)
    else
      table.insert(out, -1)
      table.insert(out, -1)
    end
  else
    table.insert(out, -1)
    table.insert(out, -1)
  end
end
return out
";

const REBUILD: &str = r"
if redis.call('HEXISTS', KEYS[1], 'v') == 0 then
  redis.call('HSET', KEYS[1], 'v', ARGV[1], 'n', ARGV[2], 'a', ARGV[4])
  if tonumber(ARGV[3]) > 0 then redis.call('EXPIRE', KEYS[1], tonumber(ARGV[3])) end
end
return 1
";

const INVALIDATE: &str = r"
local at = 1
for i = 1, #KEYS do
  local n = tonumber(ARGV[at])
  at = at + 1
  for _ = 1, n do
    redis.call('ZREM', KEYS[i] .. ':f', ARGV[at])
    redis.call('HDEL', KEYS[i] .. ':r', ARGV[at])
    at = at + 1
  end
  redis.call('DEL', KEYS[i])
end
return 1
";

static ADMIT_SCRIPT: std::sync::LazyLock<Script> = std::sync::LazyLock::new(|| Script::new(ADMIT));
static CHARGE_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(CHARGE));
static REBUILD_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(REBUILD));
static INVALIDATE_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(INVALIDATE));
static EXTEND_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(EXTEND));

#[derive(Debug, Default)]
pub struct Distrusted {
    counters: Mutex<HashMap<String, Doubt>>,
}

#[derive(Debug, Default)]
struct Doubt {
    until: i64,
    requests: Vec<String>,
}

impl Distrusted {
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Doubt>> {
        self.counters.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn add(&self, now: i64, doubts: Vec<(String, Doubt)>) {
        let mut counters = self.lock();
        counters.retain(|_, d| d.until > now);
        for (key, doubt) in doubts {
            let known = counters.entry(key).or_default();
            known.until = known.until.max(doubt.until);
            known.requests.extend(doubt.requests);
        }
    }

    fn take(&self, keys: &[String]) -> Vec<(String, Doubt)> {
        let mut counters = self.lock();
        keys.iter()
            .filter_map(|k| counters.remove(k).map(|d| (k.clone(), d)))
            .collect()
    }
}

fn script_type(kind: Kind) -> i64 {
    match kind {
        Kind::RequestRate => 1,
        Kind::TokenRate => 2,
        Kind::TokenBudget | Kind::SpendBudget => 3,
    }
}

fn ttl(period: Period) -> i64 {
    match period {
        Period::Minute => 120,
        Period::Day => 3 * 86_400,
        Period::Month | Period::Total => 40 * 86_400,
    }
}

pub fn member_of(c: &Caller) -> Option<Subject> {
    c.user_id
        .map(Subject::User)
        .or(c.service_account_id.map(Subject::ServiceAccount))
}

pub async fn project_limits(
    state: &AppState,
    project: Uuid,
) -> Result<Arc<Vec<Limit>>, StoreError> {
    let pg = state.pg.clone();
    let found = state
        .caches
        .project_limits
        .get_or_load(&project.to_string(), || async move {
            let rows = store::list(&pg, project).await?;
            rows.iter()
                .map(store::LimitRow::limit)
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        })
        .await?;
    Ok(found.unwrap_or_default())
}

pub async fn forget(state: &AppState, project: Uuid) {
    state
        .caches
        .project_limits
        .invalidate(&project.to_string())
        .await;
}

fn noun(subject: Subject) -> &'static str {
    match subject.level() {
        Level::Project => "the project's",
        Level::Member => "the member's",
        Level::Key => "this key's",
    }
}

enum Why {
    Over(i64),
    Busy,
    TooLarge,
}

fn refusal(l: &Limit, now: OffsetDateTime, why: Why) -> Refusal {
    let scope = if l.model.is_some() {
        " for this model"
    } else {
        ""
    };
    match why {
        Why::Busy => Refusal {
            status: 429,
            message: format!(
                "{} {}{scope} is nearly used up by requests in progress; retry in a moment.",
                capitalise(noun(l.subject)),
                l.describe()
            ),
            retry_after: Some(Duration::from_secs(1)),
            param: None,
        },
        Why::TooLarge => Refusal {
            status: 429,
            message: format!(
                "This request could exceed {} {}{scope}; ask for fewer output tokens.",
                noun(l.subject),
                l.describe()
            ),
            retry_after: None,
            param: None,
        },
        Why::Over(retry) if l.kind.is_rate() => {
            let secs = u64::try_from(retry.max(1)).unwrap_or(1);
            Refusal {
                status: 429,
                message: format!(
                    "Rate limit reached: {} {}{scope}; retry in {}.",
                    noun(l.subject),
                    l.describe(),
                    crate::auth::throttle::wait(secs)
                ),
                retry_after: Some(Duration::from_secs(secs)),
                param: None,
            }
        }
        Why::Over(_) => {
            let resets = l.period.resets_at(now).map_or_else(
                || "it does not reset, so raise the limit to continue.".to_owned(),
                |t| {
                    format!(
                        "it resets at {}.",
                        t.format(&time::format_description::well_known::Rfc3339)
                            .unwrap_or_default()
                    )
                },
            );
            Refusal {
                status: 429,
                message: format!(
                    "Budget used up: {} {}{scope}; {resets}",
                    noun(l.subject),
                    l.describe()
                ),
                retry_after: None,
                param: None,
            }
        }
    }
}

fn unpriced_refusal(subject: Subject, u: &Unpriced) -> Refusal {
    let feature = &u.feature;
    let message = match subject.level() {
        Level::Project => {
            format!("The project has a spend budget that can't price `{feature}`; ask an admin.")
        }
        Level::Member => {
            format!("The member has a spend budget that can't price `{feature}`; ask an admin.")
        }
        Level::Key => {
            format!("This key has a spend budget that can't price `{feature}`; ask an admin.")
        }
    };
    Refusal {
        status: 403,
        message,
        retry_after: None,
        param: Some(u.param),
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

const AVERAGE_DAYS: i64 = 30;

async fn rebuild(
    state: &AppState,
    project: Uuid,
    l: &Limit,
    now: OffsetDateTime,
) -> Result<(), LimitsError> {
    let units = |tokens: i64, cost: rust_decimal::Decimal| match l.kind {
        Kind::SpendBudget => kaeso_core::cost::nano_usd(cost),
        _ => tokens,
    };
    let since = l.period.start(now);
    let used = store::used(&state.pg, project, l.subject, l.model, since).await?;
    let (p_tokens, p_cost, p_requests) = state.usage.pending(project, l.subject, l.model, since);
    let value = units(used.tokens.saturating_add(p_tokens), used.cost + p_cost);
    let count = used.succeeded.saturating_add(p_requests);
    let average = if count == 0 {
        let window =
            now.to_offset(time::UtcOffset::UTC).date() - time::Duration::days(AVERAGE_DAYS);
        let history = store::used(&state.pg, project, l.subject, l.model, Some(window)).await?;
        if history.succeeded > 0 {
            let total = units(history.tokens, history.cost);
            total.saturating_add(history.succeeded - 1) / history.succeeded
        } else {
            0
        }
    } else {
        0
    };
    let mut conn = state.valkey.conn();
    REBUILD_SCRIPT
        .key(state.valkey.key(&l.counter(now)))
        .arg(value)
        .arg(count)
        .arg(ttl(l.period))
        .arg(average)
        .invoke_async::<i64>(&mut conn)
        .await?;
    Ok(())
}

async fn rebuild_once(
    state: &AppState,
    project: Uuid,
    l: &Limit,
    now: OffsetDateTime,
) -> Result<(), Arc<LimitsError>> {
    let key = state.valkey.key(&l.counter(now));
    let (st, limit) = (state.clone(), l.clone());
    state
        .rebuilds
        .run(key, move || async move {
            rebuild(&st, project, &limit, now).await
        })
        .await
}

#[derive(Debug, Default)]
pub struct ValkeyLimits;

impl Limits for ValkeyLimits {
    fn admit<'a>(
        &'a self,
        state: &'a AppState,
        a: &'a Admission,
    ) -> BoxFuture<'a, Result<Reservation, Refusal>> {
        Box::pin(async move {
            let all = match project_limits(state, a.caller.project_id).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(error = %e, "cannot load limits; refusing the request");
                    return Err(Refusal {
                        status: 503,
                        message:
                            "Kaeso cannot check this project's limits right now; try again shortly."
                                .into(),
                        retry_after: None,
                        param: None,
                    });
                }
            };
            let applicable = limits::applicable(
                &all,
                a.caller.project_id,
                member_of(&a.caller),
                a.caller.key_id,
                a.catalogue_model_id,
            );
            if applicable.is_empty() {
                return Ok(Reservation::default());
            }
            if let Some(u) = &a.unpriced
                && let Some(l) = applicable
                    .iter()
                    .filter(|l| l.kind == Kind::SpendBudget)
                    .max_by_key(|l| l.subject.level())
            {
                metrics::counter!(crate::metrics::LIMIT_REFUSALS, "kind" => l.kind.as_str(), "level" => l.subject.level().as_str()).increment(1);
                return Err(unpriced_refusal(l.subject, u));
            }
            if let Err(e) = settle(state, &applicable, a.at).await {
                return fail(&applicable, &e);
            }
            for attempt in 0..2 {
                let refused = |i: usize, why: Why| {
                    let l = applicable.get(i).copied().ok_or_else(unavailable)?;
                    metrics::counter!(crate::metrics::LIMIT_REFUSALS, "kind" => l.kind.as_str(), "level" => l.subject.level().as_str()).increment(1);
                    Err(refusal(l, a.at, why))
                };
                match run_admit(state, a, &applicable).await {
                    Ok(Admit::Pass) => return Ok(reservation(state, a, &applicable)),
                    Ok(Admit::Refused(i, retry)) => return refused(i, Why::Over(retry)),
                    Ok(Admit::Busy(i)) => return refused(i, Why::Busy),
                    Ok(Admit::TooLarge(i)) => return refused(i, Why::TooLarge),
                    Ok(Admit::Missing(list)) if attempt == 0 => {
                        for i in list {
                            if let Some(l) = applicable.get(i)
                                && let Err(e) =
                                    rebuild_once(state, a.caller.project_id, l, a.at).await
                            {
                                return fail(&applicable, &e);
                            }
                        }
                    }
                    Ok(Admit::Missing(_)) => {
                        return fail(&applicable, &LimitsError::NotRebuilt);
                    }
                    Err(e) => return fail(&applicable, &e),
                }
            }
            fail(&applicable, &LimitsError::NotRebuilt)
        })
    }
}

fn unavailable() -> Refusal {
    Refusal {
        status: 503,
        message: "Kaeso cannot check this project's hard budgets right now; try again shortly."
            .into(),
        retry_after: None,
        param: None,
    }
}

fn reserves(l: &Limit) -> bool {
    l.kind == Kind::TokenRate || (!l.kind.is_rate() && l.enforced())
}

fn reservation(state: &AppState, a: &Admission, applicable: &[&Limit]) -> Reservation {
    Reservation {
        counters: applicable
            .iter()
            .filter(|l| reserves(l))
            .map(|l| state.valkey.key(&l.counter(a.at)))
            .collect(),
        ttl: in_flight_ttl(a.timeout),
    }
}

pub async fn extend(
    state: &AppState,
    held: &Reservation,
    request_id: &str,
) -> Result<(), LimitsError> {
    if !held.is_held() {
        return Ok(());
    }
    let until = state.clock.now().unix_timestamp().saturating_add(held.ttl);
    let mut script = EXTEND_SCRIPT.prepare_invoke();
    for key in &held.counters {
        script.key(key);
    }
    script.arg(request_id).arg(until).arg(held.ttl);
    let mut conn = state.valkey.conn();
    tokio::time::timeout(VALKEY_DEADLINE, script.invoke_async::<i64>(&mut conn))
        .await
        .map_err(|_| LimitsError::Timeout)??;
    Ok(())
}

fn fail(applicable: &[&Limit], error: &LimitsError) -> Result<Reservation, Refusal> {
    metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "limits").increment(1);
    if applicable.iter().any(|l| !l.kind.is_rate() && l.hard) {
        tracing::warn!(
            error = %error,
            "limit counters unavailable; refusing a request under a hard budget"
        );
        Err(unavailable())
    } else {
        tracing::warn!(
            error = %error,
            "limit counters unavailable; the request passes unchecked"
        );
        metrics::counter!(crate::metrics::FAIL_OPEN, "reason" => "valkey").increment(1);
        Ok(Reservation::default())
    }
}

enum Admit {
    Pass,
    Refused(usize, i64),
    Busy(usize),
    TooLarge(usize),
    Missing(Vec<usize>),
}

async fn run_admit(
    state: &AppState,
    a: &Admission,
    applicable: &[&Limit],
) -> Result<Admit, LimitsError> {
    let now = a.at.unix_timestamp();
    let mut script = ADMIT_SCRIPT.prepare_invoke();
    script.arg(now).arg(a.request_id.to_string());
    for l in applicable {
        script
            .key(state.valkey.key(&l.counter(a.at)))
            .arg(script_type(l.kind))
            .arg(l.units())
            .arg(if l.enforced() { "1" } else { "0" })
            .arg(a.worst.units(l.kind))
            .arg(if a.worst.bounded { "1" } else { "0" });
    }
    script.arg(in_flight_ttl(a.timeout));
    let mut conn = state.valkey.conn();
    let out: Vec<i64> = tokio::time::timeout(VALKEY_DEADLINE, script.invoke_async(&mut conn))
        .await
        .map_err(|_| LimitsError::Timeout)??;
    let index = |i: i64| usize::try_from(i - 1).unwrap_or(usize::MAX);
    Ok(match out.as_slice() {
        [0] => Admit::Pass,
        [1, i, retry] => Admit::Refused(index(*i), *retry),
        [3, i] => Admit::Busy(index(*i)),
        [4, i] => Admit::TooLarge(index(*i)),
        [2, rest @ ..] => Admit::Missing(rest.iter().map(|i| index(*i)).collect()),
        other => return Err(LimitsError::Protocol(other.to_vec())),
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Charge {
    pub tokens: i64,
    pub nano_usd: i64,
    pub succeeded: bool,
}

pub async fn charge(
    state: &AppState,
    caller: &Caller,
    catalogue_model_id: Uuid,
    request_id: &str,
    at: OffsetDateTime,
    usage: Charge,
) -> Result<Vec<(Limit, i64, i64)>, LimitsError> {
    let all = project_limits(state, caller.project_id).await?;
    let applicable = limits::applicable(
        &all,
        caller.project_id,
        member_of(caller),
        caller.key_id,
        catalogue_model_id,
    );
    let charged: Vec<&Limit> = applicable
        .into_iter()
        .filter(|l| l.kind != Kind::RequestRate)
        .collect();
    if charged.is_empty() {
        return Ok(Vec::new());
    }
    match run_charge(state, &charged, request_id, at, usage).await {
        Ok(moved) => Ok(moved),
        Err(e) => {
            distrust(state, &charged, request_id, at).await;
            Err(e)
        }
    }
}

async fn run_charge(
    state: &AppState,
    charged: &[&Limit],
    request_id: &str,
    at: OffsetDateTime,
    usage: Charge,
) -> Result<Vec<(Limit, i64, i64)>, LimitsError> {
    let mut script = CHARGE_SCRIPT.prepare_invoke();
    script
        .arg(state.clock.now().unix_timestamp())
        .arg(request_id)
        .arg(if usage.succeeded { "1" } else { "0" });
    for l in charged {
        let add = match l.kind {
            Kind::SpendBudget => usage.nano_usd,
            _ => usage.tokens,
        };
        script
            .key(state.valkey.key(&l.counter(at)))
            .arg(script_type(l.kind))
            .arg(add)
            .arg(ttl(l.period));
    }
    let mut conn = state.valkey.conn();
    let out: Vec<i64> = tokio::time::timeout(VALKEY_DEADLINE, script.invoke_async(&mut conn))
        .await
        .map_err(|_| LimitsError::Timeout)??;
    Ok(charged
        .iter()
        .zip(out.chunks(2))
        .filter_map(|(l, pair)| match pair {
            [before, after] if *before >= 0 => Some(((*l).clone(), *before, *after)),
            _ => None,
        })
        .collect())
}

async fn distrust(state: &AppState, charged: &[&Limit], request_id: &str, at: OffsetDateTime) {
    let now = state.clock.now().unix_timestamp();
    let doubts: Vec<(String, Doubt)> = charged
        .iter()
        .filter(|l| !l.kind.is_rate())
        .map(|l| {
            (
                state.valkey.key(&l.counter(at)),
                Doubt {
                    until: now.saturating_add(ttl(l.period)),
                    requests: vec![request_id.to_owned()],
                },
            )
        })
        .collect();
    if doubts.is_empty() {
        return;
    }
    if let Err(e) = invalidate(state, &doubts).await {
        tracing::warn!(error = %e, "cannot drop the budget counters of a failed charge; they are dropped before the next request they limit");
        state.distrusted.add(now, doubts);
    }
}

async fn settle(
    state: &AppState,
    applicable: &[&Limit],
    at: OffsetDateTime,
) -> Result<(), LimitsError> {
    if state.distrusted.is_empty() {
        return Ok(());
    }
    let keys: Vec<String> = applicable
        .iter()
        .filter(|l| !l.kind.is_rate())
        .map(|l| state.valkey.key(&l.counter(at)))
        .collect();
    let doubts = state.distrusted.take(&keys);
    if doubts.is_empty() {
        return Ok(());
    }
    if let Err(e) = invalidate(state, &doubts).await {
        state
            .distrusted
            .add(state.clock.now().unix_timestamp(), doubts);
        return Err(e);
    }
    Ok(())
}

async fn invalidate(state: &AppState, doubts: &[(String, Doubt)]) -> Result<(), LimitsError> {
    let mut script = INVALIDATE_SCRIPT.prepare_invoke();
    for (key, doubt) in doubts {
        script.key(key).arg(doubt.requests.len());
        for request in &doubt.requests {
            script.arg(request);
        }
    }
    let mut conn = state.valkey.conn();
    tokio::time::timeout(VALKEY_DEADLINE, script.invoke_async::<i64>(&mut conn))
        .await
        .map_err(|_| LimitsError::Timeout)??;
    Ok(())
}

pub async fn current(
    state: &AppState,
    project: Uuid,
    l: &Limit,
    now: OffsetDateTime,
) -> Option<i64> {
    if l.kind.is_rate() {
        let mut conn = state.valkey.conn();
        let fields: std::collections::HashMap<String, i64> = redis::cmd("HGETALL")
            .arg(state.valkey.key(&l.counter(now)))
            .query_async(&mut conn)
            .await
            .map_err(|e| unreadable(&LimitsError::Valkey(e)))
            .ok()?;
        let cutoff = now.unix_timestamp() - 60;
        return Some(
            fields
                .iter()
                .filter(|(s, _)| s.parse::<i64>().is_ok_and(|s| s > cutoff))
                .map(|(_, v)| *v)
                .sum(),
        );
    }
    let mut conn = state.valkey.conn();
    let v: Option<i64> = redis::cmd("HGET")
        .arg(state.valkey.key(&l.counter(now)))
        .arg("v")
        .query_async(&mut conn)
        .await
        .map_err(|e| unreadable(&LimitsError::Valkey(e)))
        .ok()
        .flatten();
    if v.is_some() {
        return v;
    }
    let used = store::used(&state.pg, project, l.subject, l.model, l.period.start(now))
        .await
        .map_err(|e| unreadable(&LimitsError::Store(e)))
        .ok()?;
    Some(match l.kind {
        Kind::SpendBudget => kaeso_core::cost::nano_usd(used.cost),
        _ => used.tokens,
    })
}

fn unreadable(e: &LimitsError) {
    if matches!(e, LimitsError::Valkey(_)) {
        metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "limit_usage")
            .increment(1);
    }
    tracing::warn!(error = %e, "cannot read what a limit has used");
}
