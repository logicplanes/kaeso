use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use futures::future::BoxFuture;
use kaeso_core::cost::Unpriced;
use kaeso_core::currency::RateBook;
use kaeso_core::limits::{self, Kind, Level, Limit, Period, Subject};
use kaeso_store::StoreError;
use kaeso_store::pg::limits as store;
use redis::Script;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::model_api::hooks::{Admission, Caller, Left, Limits, Refusal, Reservation};
use crate::services::exchange_rates as rates;
use crate::state::AppState;

mod flights;

pub type Rebuilds = flights::Flights<LimitsError>;

// A finishing request adds its usage to the in-memory batch and then charges the counters, which
// add nothing to a counter that is missing. A rebuild reads the batch and then sets the counter.
// Run between those two steps, a rebuild would miss the request (read before the add, set after
// the charge found nothing) or count it twice (read after the add, set before the charge). The
// two steps hold this gate shared, and a rebuild's read and set hold it alone, so a rebuild sees
// either both steps or neither.
#[derive(Debug, Default)]
pub struct Charging(tokio::sync::RwLock<()>);

impl Charging {
    pub async fn finishing(&self) -> tokio::sync::RwLockReadGuard<'_, ()> {
        self.0.read().await
    }

    async fn rebuilding(&self) -> tokio::sync::RwLockWriteGuard<'_, ()> {
        self.0.write().await
    }
}

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

// A counter's open reservations live in `<key>:f` (expiry by request) and `<key>:r` (amount by
// request); `<key>:j` names those that started videos or image generations still being made.
// `#held` and `#n` in `:r` keep their total and count, and `#jobheld` and `#jobs` those of the
// jobs, so an admission costs the same with 5 or 5,000 streams open; a count that disagrees with
// `:f`, as while an older release still writes, or a total a minute old is summed again from the
// reservations themselves.
const RELEASE: &str = r"
local function release(key, req)
  local r = key .. ':r'
  local amount = tonumber(redis.call('HGET', r, req) or '0')
  if redis.call('ZREM', key .. ':f', req) == 1 and redis.call('HEXISTS', r, '#n') == 1 then
    redis.call('HINCRBY', r, '#held', -amount)
    if redis.call('HINCRBY', r, '#n', -1) <= 0 then
      redis.call('HDEL', r, '#held', '#n', '#at', '#jobs', '#jobheld')
    end
  end
  redis.call('HDEL', r, req)
  if redis.call('SREM', key .. ':j', req) == 1 and redis.call('HEXISTS', r, '#jobs') == 1 then
    redis.call('HINCRBY', r, '#jobs', -1)
    redis.call('HINCRBY', r, '#jobheld', -amount)
  end
end
";

const ADMIT: &str = r"
local now = tonumber(ARGV[1])
local req = ARGV[2]
local ttl = tonumber(ARGV[#ARGV])
local missing = {}
local own = {}
local used = {}
local resets = {}
local function flying(key)
  local f = key .. ':f'
  local r = key .. ':r'
  local held = tonumber(redis.call('HGET', r, '#held') or '0')
  local counted = tonumber(redis.call('HGET', r, '#n') or '-1')
  local summed = tonumber(redis.call('HGET', r, '#at') or '0')
  local jobs = tonumber(redis.call('HGET', r, '#jobs') or '-1')
  local by_jobs = tonumber(redis.call('HGET', r, '#jobheld') or '0')
  local gone = redis.call('ZRANGEBYSCORE', f, '-inf', now)
  for _, id in ipairs(gone) do
    local amount = tonumber(redis.call('HGET', r, id) or '0')
    held = held - amount
    redis.call('HDEL', r, id)
    if redis.call('SREM', key .. ':j', id) == 1 then
      jobs = jobs - 1
      by_jobs = by_jobs - amount
    end
  end
  if #gone > 0 then
    redis.call('ZREMRANGEBYSCORE', f, '-inf', now)
    counted = counted - #gone
  end
  local n = redis.call('ZCARD', f)
  local stale = n ~= counted or held < 0 or summed + 60 <= now
  if stale then
    held = 0
    for _, id in ipairs(redis.call('ZRANGE', f, 0, -1)) do
      held = held + tonumber(redis.call('HGET', r, id) or '0')
    end
    summed = now
  end
  if stale or jobs < 0 or by_jobs < 0 then
    jobs, by_jobs = 0, 0
    for _, id in ipairs(redis.call('SMEMBERS', key .. ':j')) do
      if redis.call('ZSCORE', f, id) then
        jobs = jobs + 1
        by_jobs = by_jobs + tonumber(redis.call('HGET', r, id) or '0')
      else
        redis.call('SREM', key .. ':j', id)
      end
    end
  end
  if n > 0 then
    redis.call('HSET', r, '#held', held, '#n', n, '#at', summed, '#jobs', jobs, '#jobheld', by_jobs)
    if redis.call('TTL', r) == -1 then redis.call('EXPIRE', r, ttl) end
  else
    redis.call('HDEL', r, '#held', '#n', '#at', '#jobs', '#jobheld')
  end
  return n, held, jobs, by_jobs
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
    -- A request rate counts this request below, so it is used already.
    if t == 1 then
      used[i] = sum + 1
      if oldest == nil then resets[i] = 60 else resets[i] = retry end
    else
      used[i] = sum
      if oldest == nil then resets[i] = 60 else resets[i] = retry end
    end
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
      used[i] = v
      local count = tonumber(redis.call('HGET', key, 'n') or '0')
      local avg = tonumber(redis.call('HGET', key, 'a') or '0')
      if count > 0 then avg = math.ceil(v / count) end
      local mine = math.max(avg, reserve)
      if bounded and v + reserve > units then return {4, i} end
      local n, held, jobs, by_jobs = flying(key)
      if n > 0 then
        if not bounded and avg <= 0 and n > jobs then return {3, i} end
        if v + held + mine > units then
          if v + held - by_jobs + mine <= units then return {5, i} end
          return {3, i}
        end
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
    local before = tonumber(redis.call('HGET', r, req) or '0')
    if redis.call('ZADD', f, now + ttl, req) == 1 then redis.call('HINCRBY', r, '#n', 1) end
    redis.call('HSET', r, req, own[i])
    redis.call('HINCRBY', r, '#held', own[i] - before)
    if redis.call('TTL', f) < ttl then redis.call('EXPIRE', f, ttl) end
    if redis.call('TTL', r) < ttl then redis.call('EXPIRE', r, ttl) end
  end
end
-- What each limit has used and when its rate window frees it; -1 where there is nothing to say.
local out = {0}
for i = 1, #KEYS do
  table.insert(out, used[i] or -1)
  table.insert(out, resets[i] or -1)
end
return out
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
    release(key, req)
    if add > 0 then
      redis.call('HINCRBY', key, tostring(now), add)
      redis.call('EXPIRE', key, 120)
    end
    table.insert(out, -1)
    table.insert(out, -1)
  elseif t == 3 then
    release(key, req)
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

// A started video or image generation keeps what it reserved of each budget, under its starting
// request's id, until the job is billed; `KEYS[#KEYS]` lists the counters it holds. Rates and
// reservations of nothing are released as usual.
const HOLD: &str = r"
local now = tonumber(ARGV[1])
local req = ARGV[2]
local hold = tonumber(ARGV[3])
local held = KEYS[#KEYS]
local kept = 0
for i = 1, #KEYS - 1 do
  local key = KEYS[i]
  local f = key .. ':f'
  local r = key .. ':r'
  local amount = tonumber(redis.call('HGET', r, req) or '0')
  if ARGV[3 + i] == '3' and amount > 0 and redis.call('ZSCORE', f, req) then
    redis.call('ZADD', f, 'XX', now + hold, req)
    if redis.call('TTL', f) < hold then redis.call('EXPIRE', f, hold) end
    if redis.call('TTL', r) < hold then redis.call('EXPIRE', r, hold) end
    if redis.call('SADD', key .. ':j', req) == 1 and redis.call('HEXISTS', r, '#jobs') == 1 then
      redis.call('HINCRBY', r, '#jobs', 1)
      redis.call('HINCRBY', r, '#jobheld', amount)
    end
    if redis.call('TTL', key .. ':j') < hold then redis.call('EXPIRE', key .. ':j', hold) end
    redis.call('SADD', held, key)
    kept = kept + 1
  else
    release(key, req)
  end
end
if kept > 0 and redis.call('TTL', held) < hold then redis.call('EXPIRE', held, hold) end
return kept
";

const EXTEND_HELD: &str = r"
local now = tonumber(ARGV[1])
local req = ARGV[2]
local hold = tonumber(ARGV[3])
local held = KEYS[#KEYS]
for i = 1, #KEYS - 1 do
  local f = KEYS[i] .. ':f'
  if redis.call('ZSCORE', f, req) then
    redis.call('ZADD', f, 'XX', now + hold, req)
    if redis.call('TTL', f) < hold then redis.call('EXPIRE', f, hold) end
    local r = KEYS[i] .. ':r'
    if redis.call('TTL', r) < hold then redis.call('EXPIRE', r, hold) end
    local j = KEYS[i] .. ':j'
    if redis.call('TTL', j) < hold then redis.call('EXPIRE', j, hold) end
  end
end
if redis.call('TTL', held) < hold then redis.call('EXPIRE', held, hold) end
return 1
";

const RELEASE_HELD: &str = r"
for i = 1, #KEYS - 1 do
  release(KEYS[i], ARGV[1])
end
redis.call('DEL', KEYS[#KEYS])
return 1
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
    release(KEYS[i], ARGV[at])
    at = at + 1
  end
  redis.call('DEL', KEYS[i])
end
return 1
";

const RELEASE_ALL: &str = r"
for i = 1, #KEYS do
  release(KEYS[i], ARGV[1])
end
return 1
";

static ADMIT_SCRIPT: std::sync::LazyLock<Script> = std::sync::LazyLock::new(|| Script::new(ADMIT));
static RELEASE_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(&[RELEASE, RELEASE_ALL].concat()));
static CHARGE_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(&[RELEASE, CHARGE].concat()));
static REBUILD_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(REBUILD));
static INVALIDATE_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(&[RELEASE, INVALIDATE].concat()));
static EXTEND_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(EXTEND));
static HOLD_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(&[RELEASE, HOLD].concat()));
static EXTEND_HELD_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(EXTEND_HELD));
static RELEASE_HELD_SCRIPT: std::sync::LazyLock<Script> =
    std::sync::LazyLock::new(|| Script::new(&[RELEASE, RELEASE_HELD].concat()));

// How long a started job holds its reservations without news of it. The background job that
// asks providers about unbilled jobs renews the hold of every one still running, so only a job
// Kaeso can no longer reach lets its hold lapse.
pub const JOB_HOLD: Duration = Duration::from_secs(3600);

// The background job that bills ended jobs runs every minute, so asking sooner rarely helps.
const JOB_RETRY: Duration = Duration::from_secs(60);

fn job_retry() -> i64 {
    i64::try_from(JOB_RETRY.as_secs()).unwrap_or(60)
}

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
        Kind::RequestRate | Kind::ToolCallRate => 1,
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
        .get_or_refresh(&project.to_string(), move || async move {
            let rows = store::list(&pg, project).await?;
            rows.iter()
                .map(store::LimitRow::limit)
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        })
        .await?;
    Ok(found.unwrap_or_default())
}

// A tool call counts when it is admitted, in the same per-second counter as a request rate, so
// the limits page reads it the same way. Like model requests, it passes unchecked when the
// counters can't be reached.
pub async fn admit_tool_call(
    state: &AppState,
    project: Uuid,
    member: Option<Subject>,
    key: Option<Uuid>,
    at: OffsetDateTime,
) -> Result<(), Refusal> {
    let all = match project_limits(state, project).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, "limits unavailable; the tool call passes unchecked");
            metrics::counter!(crate::metrics::FAIL_OPEN, "reason" => "limits").increment(1);
            return Ok(());
        }
    };
    let applicable = kaeso_core::limits::applicable_to_tools(&all, project, member, key);
    count_one(state, &applicable, at).await
}

// A request that reaches a provider outside the model pipeline, such as asking how a video job
// stands or listing voices, spends the provider's quota with the organization's key, so it counts
// against the request rates of its project, member and key, and of its model's limits when it
// has one. It holds no reservation and passes unchecked when the counters can't be reached.
pub async fn admit_follow_up(
    state: &AppState,
    caller: &Caller,
    model: Option<Uuid>,
    at: OffsetDateTime,
) -> Result<(), Refusal> {
    let all = match project_limits(state, caller.project_id).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, "limits unavailable; the request passes unchecked");
            metrics::counter!(crate::metrics::FAIL_OPEN, "reason" => "limits").increment(1);
            return Ok(());
        }
    };
    let applicable = kaeso_core::limits::applicable_to_follow_ups(
        &all,
        caller.project_id,
        member_of(caller),
        caller.key_id,
        model,
    );
    count_one(state, &applicable, at).await
}

// Asked before a request's guardrails run, so a caller over its request rate can't make Kaeso
// call them; counts nothing, as the request is counted when it is admitted or blocked.
pub async fn within_request_rates(
    state: &AppState,
    caller: &Caller,
    at: OffsetDateTime,
) -> Result<(), Refusal> {
    let all = match project_limits(state, caller.project_id).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, "limits unavailable; the request passes unchecked");
            metrics::counter!(crate::metrics::FAIL_OPEN, "reason" => "limits").increment(1);
            return Ok(());
        }
    };
    let applicable: Vec<&Limit> = kaeso_core::limits::applicable_to_follow_ups(
        &all,
        caller.project_id,
        member_of(caller),
        caller.key_id,
        None,
    )
    .into_iter()
    .filter(|l| l.enforced())
    .collect();
    if applicable.is_empty() {
        return Ok(());
    }
    let now = at.unix_timestamp();
    let mut pipe = redis::pipe();
    for l in &applicable {
        pipe.cmd("HGETALL").arg(state.valkey.key(&l.counter(at)));
    }
    let mut conn = state.valkey.conn();
    let windows: Vec<HashMap<String, i64>> =
        match tokio::time::timeout(VALKEY_DEADLINE, pipe.query_async(&mut conn)).await {
            Ok(Ok(w)) => w,
            Ok(Err(_)) | Err(_) => {
                tracing::warn!("rate counters unavailable; the request passes unchecked");
                metrics::counter!(crate::metrics::FAIL_OPEN, "reason" => "valkey").increment(1);
                return Ok(());
            }
        };
    let cutoff = now - 60;
    let book = rates::book(state);
    for (l, window) in applicable.iter().zip(&windows) {
        let live: Vec<(i64, i64)> = window
            .iter()
            .filter_map(|(s, n)| s.parse::<i64>().ok().map(|s| (s, *n)))
            .filter(|(s, _)| *s > cutoff)
            .collect();
        let used: i64 = live.iter().map(|(_, n)| n).sum();
        if used >= l.units(&book) {
            let oldest = live.iter().map(|(s, _)| *s).min().unwrap_or(now);
            metrics::counter!(crate::metrics::LIMIT_REFUSALS, "kind" => l.kind.as_str(), "level" => l.subject.level().as_str()).increment(1);
            return Err(refusal(l, at, Why::Over(oldest + 60 - now)));
        }
    }
    Ok(())
}

async fn count_one(
    state: &AppState,
    applicable: &[&Limit],
    at: OffsetDateTime,
) -> Result<(), Refusal> {
    if applicable.is_empty() {
        return Ok(());
    }
    let now = at.unix_timestamp();
    let mut pipe = redis::pipe();
    for l in applicable {
        let counter = state.valkey.key(&l.counter(at));
        pipe.cmd("HINCRBY")
            .arg(&counter)
            .arg(now)
            .arg(1)
            .ignore()
            .cmd("EXPIRE")
            .arg(&counter)
            .arg(120)
            .ignore()
            .cmd("HGETALL")
            .arg(&counter);
    }
    let mut conn = state.valkey.conn();
    let windows: Vec<HashMap<String, i64>> =
        match tokio::time::timeout(VALKEY_DEADLINE, pipe.query_async(&mut conn)).await {
            Ok(Ok(w)) => w,
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "rate counters unavailable; the call passes unchecked");
                metrics::counter!(crate::metrics::FAIL_OPEN, "reason" => "valkey").increment(1);
                return Ok(());
            }
            Err(_) => {
                tracing::warn!("rate counters timed out; the call passes unchecked");
                metrics::counter!(crate::metrics::FAIL_OPEN, "reason" => "valkey").increment(1);
                return Ok(());
            }
        };
    let cutoff = now - 60;
    let book = rates::book(state);
    let mut refused = None;
    let mut undo = redis::pipe();
    for (l, window) in applicable.iter().zip(&windows) {
        let counter = state.valkey.key(&l.counter(at));
        let seconds: Vec<(i64, i64)> = window
            .iter()
            .filter_map(|(s, n)| s.parse::<i64>().ok().map(|s| (s, *n)))
            .collect();
        for (s, _) in seconds.iter().filter(|(s, _)| *s <= cutoff) {
            undo.cmd("HDEL").arg(&counter).arg(*s).ignore();
        }
        let used: i64 = seconds
            .iter()
            .filter(|(s, _)| *s > cutoff)
            .map(|(_, n)| n)
            .sum();
        if refused.is_none() && used > l.units(&book) {
            let oldest = seconds
                .iter()
                .filter(|(s, _)| *s > cutoff)
                .map(|(s, _)| *s)
                .min()
                .unwrap_or(now);
            metrics::counter!(crate::metrics::LIMIT_REFUSALS, "kind" => l.kind.as_str(), "level" => l.subject.level().as_str()).increment(1);
            refused = Some(refusal(l, at, Why::Over(oldest + 60 - now)));
        }
    }
    if refused.is_some() {
        for l in applicable {
            undo.cmd("HINCRBY")
                .arg(state.valkey.key(&l.counter(at)))
                .arg(now)
                .arg(-1)
                .ignore();
        }
    }
    let _: Result<(), _> = undo.query_async(&mut conn).await;
    refused.map_or(Ok(()), Err)
}

/// Whether a session that has run up `spent_nano_usd` so far still fits every hard spend budget
/// of its project, member, key and model, as their counters stand now.
pub async fn session_within_budgets(
    state: &AppState,
    caller: &Caller,
    model: Uuid,
    at: OffsetDateTime,
    spent_nano_usd: i64,
) -> Result<(), Refusal> {
    let Ok(all) = project_limits(state, caller.project_id).await else {
        return Ok(());
    };
    let applicable = limits::applicable(
        &all,
        caller.project_id,
        member_of(caller),
        caller.key_id,
        model,
    );
    for l in applicable
        .into_iter()
        .filter(|l| l.kind == Kind::SpendBudget && l.enforced())
    {
        let used = current(state, caller.project_id, l, at).await.unwrap_or(0);
        if used.saturating_add(spent_nano_usd) >= l.units(&rates::book(state)) {
            metrics::counter!(crate::metrics::LIMIT_REFUSALS, "kind" => l.kind.as_str(), "level" => l.subject.level().as_str()).increment(1);
            return Err(refusal(l, at, Why::Over(0)));
        }
    }
    Ok(())
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

#[derive(Clone, Copy)]
enum Why {
    Over(i64),
    Busy,
    HeldByJobs,
    // A transcription, a translation or a voice change is made smaller by shorter audio, not by
    // fewer output tokens.
    TooLarge { audio: bool },
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
            lasting: false,
            param: None,
            left: None,
        },
        Why::HeldByJobs => Refusal {
            status: 429,
            message: format!(
                "{} {}{scope} is held by videos still being made; retry once they have ended.",
                capitalise(noun(l.subject)),
                l.describe()
            ),
            retry_after: Some(JOB_RETRY),
            lasting: false,
            param: None,
            left: None,
        },
        Why::TooLarge { audio: false } => Refusal {
            status: 429,
            message: format!(
                "This request could exceed {} {}{scope}; ask for fewer output tokens.",
                noun(l.subject),
                l.describe()
            ),
            retry_after: None,
            lasting: true,
            param: None,
            left: None,
        },
        Why::TooLarge { audio: true } => Refusal {
            status: 429,
            message: format!(
                "This request could exceed {} {}{scope}; send shorter audio.",
                noun(l.subject),
                l.describe()
            ),
            retry_after: None,
            lasting: true,
            param: None,
            left: None,
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
                lasting: false,
                param: None,
                left: None,
            }
        }
        Why::Over(_) => {
            let reset_at = l.period.resets_at(now);
            let resets = reset_at.map_or_else(
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
                retry_after: reset_at.map(|t| until(now, t)),
                lasting: true,
                param: None,
                left: None,
            }
        }
    }
}

// The wait until a budget's period resets, in whole seconds rounded up so a retry at that
// moment finds the new period.
fn until(now: OffsetDateTime, reset: OffsetDateTime) -> Duration {
    let left = reset - now;
    let secs = left.whole_seconds().max(0) + i64::from(left.subsec_nanoseconds() > 0);
    Duration::from_secs(u64::try_from(secs.max(1)).unwrap_or(1))
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
        lasting: false,
        param: Some(u.param),
        left: None,
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
    let gate = state.charging.rebuilding().await;
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
    drop(gate);
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
                        retry_after: Some(RECHECK_AFTER),
                        lasting: false,
                        param: None,
                        left: None,
                    });
                }
            };
            let applicable: Vec<&Limit> = limits::applicable(
                &all,
                a.caller.project_id,
                member_of(&a.caller),
                a.caller.key_id,
                a.catalogue_model_id,
            )
            .into_iter()
            .filter(|l| l.kind != Kind::TokenRate || a.timeout <= Duration::from_secs(24 * 60 * 60))
            .collect();
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
            let book = rates::book(state);
            for attempt in 0..2 {
                let refused = |i: usize, why: Why| {
                    let l = applicable.get(i).copied().ok_or_else(unavailable)?;
                    metrics::counter!(crate::metrics::LIMIT_REFUSALS, "kind" => l.kind.as_str(), "level" => l.subject.level().as_str()).increment(1);
                    let mut r = refusal(l, a.at, why);
                    // Nothing is left of a limit that is used up or held by requests in flight.
                    r.left = match why {
                        Why::Over(retry) => left(l, a.at, l.units(&book), retry, &book),
                        Why::Busy => left(l, a.at, l.units(&book), 1, &book),
                        Why::HeldByJobs => left(l, a.at, l.units(&book), job_retry(), &book),
                        Why::TooLarge { .. } => None,
                    };
                    Err(r)
                };
                match run_admit(state, a, &applicable).await {
                    Ok(Admit::Pass(used)) => {
                        let mut held = reservation(state, a, &applicable);
                        held.left = applicable
                            .iter()
                            .zip(used)
                            .filter_map(|(l, (used, reset))| left(l, a.at, used, reset, &book))
                            .collect();
                        return Ok(held);
                    }
                    Ok(Admit::Refused(i, retry)) => return refused(i, Why::Over(retry)),
                    Ok(Admit::Busy(i)) => return refused(i, Why::Busy),
                    Ok(Admit::HeldByJobs(i)) => return refused(i, Why::HeldByJobs),
                    Ok(Admit::TooLarge(i)) => {
                        return refused(
                            i,
                            Why::TooLarge {
                                audio: a.sends_audio,
                            },
                        );
                    }
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

    fn release<'a>(
        &'a self,
        state: &'a AppState,
        held: &'a Reservation,
        request_id: &'a str,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Err(e) = release(state, held, request_id).await {
                metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "reservation")
                    .increment(1);
                tracing::warn!(error = %e, "cannot release a failed step's reservation; it expires with its timeout");
            }
        })
    }
}

// Valkey answers again within seconds of a blip, and rebuilding the counters takes about as
// long, so an app that waits this long and retries usually gets through.
const RECHECK_AFTER: Duration = Duration::from_secs(5);

fn unavailable() -> Refusal {
    Refusal {
        status: 503,
        message: "Kaeso cannot check this project's hard budgets right now; try again shortly."
            .into(),
        retry_after: Some(RECHECK_AFTER),
        lasting: false,
        param: None,
        left: None,
    }
}

// What the reply header says of a limit: its name, amount, what is left and when it frees up.
// Only enforced limits (rates and hard budgets) are reported; `used` is -1 for the others.
fn left(l: &Limit, at: OffsetDateTime, used: i64, reset: i64, book: &RateBook) -> Option<Left> {
    if !l.enforced() || used < 0 {
        return None;
    }
    let kind = match l.kind {
        Kind::RequestRate => "requests",
        Kind::TokenRate | Kind::TokenBudget => "tokens",
        Kind::SpendBudget => "spend",
        Kind::ToolCallRate => "tool_calls",
    };
    let reset = if l.kind.is_rate() {
        Some(reset.max(0))
    } else {
        l.period
            .resets_at(at)
            .map(|t| (t - at).whole_seconds().max(0))
    };
    Some(Left {
        name: format!(
            "{}.{kind}.{}",
            l.subject.level().as_str(),
            l.period.as_str()
        ),
        model: l.model.is_some(),
        spend: l.kind == Kind::SpendBudget,
        limit: l.units(book),
        remaining: l.units(book).saturating_sub(used).max(0),
        reset,
    })
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
        left: Vec::new(),
    }
}

pub async fn release(
    state: &AppState,
    held: &Reservation,
    request_id: &str,
) -> Result<(), LimitsError> {
    if !held.is_held() {
        return Ok(());
    }
    let mut script = RELEASE_SCRIPT.prepare_invoke();
    for key in &held.counters {
        script.key(key);
    }
    script.arg(request_id);
    let mut conn = state.valkey.conn();
    tokio::time::timeout(VALKEY_DEADLINE, script.invoke_async::<i64>(&mut conn))
        .await
        .map_err(|_| LimitsError::Timeout)??;
    Ok(())
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
    /// Each limit's used amount and rate reset, `-1` where there is nothing to say.
    Pass(Vec<(i64, i64)>),
    Refused(usize, i64),
    Busy(usize),
    HeldByJobs(usize),
    TooLarge(usize),
    Missing(Vec<usize>),
}

async fn run_admit(
    state: &AppState,
    a: &Admission,
    applicable: &[&Limit],
) -> Result<Admit, LimitsError> {
    let now = a.at.unix_timestamp();
    let book = rates::book(state);
    let mut script = ADMIT_SCRIPT.prepare_invoke();
    script.arg(now).arg(a.request_id.to_string());
    for l in applicable {
        script
            .key(state.valkey.key(&l.counter(a.at)))
            .arg(script_type(l.kind))
            .arg(l.units(&book))
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
        [0, rest @ ..] if rest.len() == applicable.len() * 2 => Admit::Pass(
            rest.as_chunks::<2>()
                .0
                .iter()
                .map(|&[u, r]| (u, r))
                .collect(),
        ),
        [1, i, retry] => Admit::Refused(index(*i), *retry),
        [3, i] => Admit::Busy(index(*i)),
        [5, i] => Admit::HeldByJobs(index(*i)),
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

/// Builds the counters of the limits an observed request is charged to, as admission does for
/// the gateway's own requests, so its charge and the alerts it crosses land. Runs before the
/// request's usage is buffered: a rebuild counts buffered usage, and would count it twice.
pub async fn prepare_observed(
    state: &AppState,
    caller: &Caller,
    catalogue_model_id: Uuid,
    at: OffsetDateTime,
) {
    let all = match project_limits(state, caller.project_id).await {
        Ok(all) => all,
        Err(e) => {
            tracing::warn!(error = %e, "cannot read the project's limits for an observed request");
            return;
        }
    };
    let applicable = limits::applicable(
        &all,
        caller.project_id,
        member_of(caller),
        caller.key_id,
        catalogue_model_id,
    );
    for l in applicable.into_iter().filter(|l| !l.kind.is_rate()) {
        if let Err(e) = rebuild_once(state, caller.project_id, l, at).await {
            tracing::warn!(error = %e, "cannot build a limit counter for an observed request");
        }
    }
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

fn held_key(state: &AppState, request_id: &str) -> String {
    state.valkey.key(&format!("jobhold:{request_id}"))
}

fn job_hold() -> i64 {
    i64::try_from(JOB_HOLD.as_secs()).unwrap_or(i64::MAX)
}

/// Keeps what the request that started a video or an image generation reserved of each budget
/// until [`release_job`], instead of releasing it when the request ends: the provider bills the
/// job later, and every other request must see that money as taken until then.
pub async fn hold_job(
    state: &AppState,
    caller: &Caller,
    catalogue_model_id: Uuid,
    request_id: &str,
    at: OffsetDateTime,
) -> Result<(), LimitsError> {
    let all = project_limits(state, caller.project_id).await?;
    let held: Vec<&Limit> = limits::applicable(
        &all,
        caller.project_id,
        member_of(caller),
        caller.key_id,
        catalogue_model_id,
    )
    .into_iter()
    .filter(|l| l.kind != Kind::RequestRate)
    .collect();
    if held.is_empty() {
        return Ok(());
    }
    let mut script = HOLD_SCRIPT.prepare_invoke();
    script
        .arg(state.clock.now().unix_timestamp())
        .arg(request_id)
        .arg(job_hold());
    for l in &held {
        script
            .key(state.valkey.key(&l.counter(at)))
            .arg(script_type(l.kind));
    }
    script.key(held_key(state, request_id));
    let mut conn = state.valkey.conn();
    tokio::time::timeout(VALKEY_DEADLINE, script.invoke_async::<i64>(&mut conn))
        .await
        .map_err(|_| LimitsError::Timeout)??;
    Ok(())
}

async fn held_counters(state: &AppState, request_id: &str) -> Result<Vec<String>, LimitsError> {
    let mut conn = state.valkey.conn();
    let counters: Vec<String> = tokio::time::timeout(
        VALKEY_DEADLINE,
        redis::cmd("SMEMBERS")
            .arg(held_key(state, request_id))
            .query_async(&mut conn),
    )
    .await
    .map_err(|_| LimitsError::Timeout)??;
    Ok(counters)
}

/// Renews the hold of a job that is still running.
pub async fn extend_job(state: &AppState, request_id: &str) -> Result<(), LimitsError> {
    let counters = held_counters(state, request_id).await?;
    if counters.is_empty() {
        return Ok(());
    }
    let mut script = EXTEND_HELD_SCRIPT.prepare_invoke();
    for key in &counters {
        script.key(key);
    }
    script
        .key(held_key(state, request_id))
        .arg(state.clock.now().unix_timestamp())
        .arg(request_id)
        .arg(job_hold());
    let mut conn = state.valkey.conn();
    tokio::time::timeout(VALKEY_DEADLINE, script.invoke_async::<i64>(&mut conn))
        .await
        .map_err(|_| LimitsError::Timeout)??;
    Ok(())
}

/// Lets go of what a job held, once it is billed or has ended without a cost.
pub async fn release_job(state: &AppState, request_id: &str) -> Result<(), LimitsError> {
    let counters = held_counters(state, request_id).await?;
    if counters.is_empty() {
        return Ok(());
    }
    let mut script = RELEASE_HELD_SCRIPT.prepare_invoke();
    for key in &counters {
        script.key(key);
    }
    script.key(held_key(state, request_id)).arg(request_id);
    let mut conn = state.valkey.conn();
    tokio::time::timeout(VALKEY_DEADLINE, script.invoke_async::<i64>(&mut conn))
        .await
        .map_err(|_| LimitsError::Timeout)??;
    Ok(())
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

pub async fn release_batch_hold(
    state: &AppState,
    caller: &Caller,
    catalogue_model_id: Uuid,
    request_id: kaeso_core::id::RequestId,
    created_at: OffsetDateTime,
    now: OffsetDateTime,
) {
    let Ok(all) = project_limits(state, caller.project_id).await else {
        return;
    };
    let applicable = limits::applicable(
        &all,
        caller.project_id,
        member_of(caller),
        caller.key_id,
        catalogue_model_id,
    );
    let mut counters = Vec::new();
    for l in applicable
        .into_iter()
        .filter(|l| !l.kind.is_rate() && l.enforced())
    {
        let start_key = state.valkey.key(&l.counter(created_at));
        counters.push(start_key.clone());
        let now_key = state.valkey.key(&l.counter(now));
        if now_key != start_key {
            counters.push(now_key);
        }
    }
    if counters.is_empty() {
        return;
    }
    let held = Reservation {
        counters,
        ttl: 0,
        left: Vec::new(),
    };
    state
        .limits
        .release(state, &held, &request_id.to_string())
        .await;
}
