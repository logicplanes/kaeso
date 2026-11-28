use std::time::Duration;

use kaeso_codecs::{HeaderList, dispatch};
use kaeso_core::catalogue::{Effective, Entry, Fields};
use kaeso_core::cost::{self, Unpriced};
use kaeso_core::estimate::tokens_of_bytes;
use kaeso_core::id::RequestId;
use kaeso_core::limits::WorstCase;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{CacheHint, CacheTtl, DialectId, KppError, Request, RequestKind, ToolDef, Usage};
use kaeso_store::pg::batches::{BatchRow, FileRow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;
use uuid::Uuid;

use super::hooks::{Admission, Caller, Reservation};
use super::pipeline;
use crate::services::stored_responses::id_hash;
use crate::state::AppState;

const BATCH_HOLD: Duration = Duration::from_secs(25 * 60 * 60);

const ENDPOINTS: &[(&str, DialectId, RequestKind)] = &[
    (
        "/v1/chat/completions",
        DialectId::OpenAiChat,
        RequestKind::Generate,
    ),
    (
        "/v1/responses",
        DialectId::OpenAiResponses,
        RequestKind::Generate,
    ),
    (
        "/v1/embeddings",
        DialectId::OpenAiEmbeddings,
        RequestKind::Embed,
    ),
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct StoredUnpriced {
    param: String,
    feature: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct BatchDemand {
    input_tokens: u64,
    max_line_input_tokens: u64,
    explicit_output_tokens: u64,
    explicit_lines: u64,
    unbounded_lines: u64,
    #[serde(default)]
    unbounded_input: bool,
    one_hour_cache: bool,
    unpriced: Option<StoredUnpriced>,
}

#[derive(Deserialize)]
struct LineEnvelope<'a> {
    #[serde(borrow)]
    body: &'a RawValue,
}

fn asks_for_one_hour_cache(request: &Request) -> bool {
    let Request::Generate(g) = request else {
        return false;
    };
    let one_hour = |c: Option<&CacheHint>| c.is_some_and(|c| c.ttl == Some(CacheTtl::OneHour));
    let top_level = g
        .extras
        .fields
        .get("cache_control")
        .and_then(|v| v.to_value().ok())
        .is_some_and(|v| v.get("ttl").and_then(Value::as_str) == Some("1h"));
    top_level
        || g.all_blocks().any(|b| one_hour(b.cache()))
        || g.tools.iter().any(|t| match t {
            ToolDef::Function(f) => one_hour(f.cache.as_ref()),
            _ => false,
        })
}

fn static_param(param: &str) -> &'static str {
    match param {
        "model" => "model",
        "tools" => "tools",
        "service_tier" => "service_tier",
        "mcp_servers" => "mcp_servers",
        "web_search_options" => "web_search_options",
        "plugins" => "plugins",
        "search_parameters" => "search_parameters",
        "anthropic-beta" => "anthropic-beta",
        _ => "body",
    }
}

fn has_item_reference(request: &Request) -> bool {
    let Request::Generate(g) = request else {
        return false;
    };
    g.all_blocks().any(|b| match b {
        kaeso_kpp::Block::Unknown(item) => item
            .raw
            .to_value()
            .ok()
            .and_then(|v| {
                v.get("type")
                    .and_then(Value::as_str)
                    .map(|t| t == "item_reference")
            })
            .unwrap_or(false),
        _ => false,
    })
}

pub(super) async fn summarize(
    state: &AppState,
    caller: &Caller,
    credential_id: Uuid,
    endpoint: &str,
    provider: &ProviderRef,
    lines: &[Vec<u8>],
) -> Value {
    let Some(&(_, dialect, kind)) = ENDPOINTS.iter().find(|(e, _, _)| *e == endpoint) else {
        return serde_json::to_value(BatchDemand::default()).unwrap_or_default();
    };
    let mut demand = BatchDemand::default();
    for raw in lines {
        let Ok(env) = serde_json::from_slice::<LineEnvelope<'_>>(raw) else {
            continue;
        };
        let body_bytes = env.body.get().as_bytes();
        let Ok(req) = dispatch::decode_request(dialect, kind, body_bytes, &HeaderList::new())
        else {
            continue;
        };
        if demand.unpriced.is_none()
            && let Some(u) = cost::unpriced(&req, dialect, provider.responses())
        {
            demand.unpriced = Some(StoredUnpriced {
                param: u.param.to_owned(),
                feature: u.feature,
            });
        }
        let mut earlier = 0_u64;
        if dialect == DialectId::OpenAiResponses
            && let Request::Generate(g) = &req
        {
            if has_item_reference(&req) || kaeso_codecs::openai_responses::has_kept_reference(g) {
                demand.unbounded_input = true;
            }
            if let Some(prev) = kaeso_codecs::openai_responses::stored_state(g)
                .previous
                .as_deref()
            {
                match kaeso_store::pg::stored_responses::find(
                    &state.pg,
                    caller.org_id,
                    caller.project_id,
                    &id_hash(prev),
                    state.clock.now(),
                )
                .await
                {
                    Ok(Some(stored)) if stored.credential_id == credential_id => {
                        earlier = u64::try_from(stored.total_tokens.max(0)).unwrap_or(0);
                    }
                    _ => {
                        demand.unbounded_input = true;
                    }
                }
            }
        }
        let line_in = tokens_of_bytes(u64::try_from(body_bytes.len()).unwrap_or(u64::MAX))
            .saturating_add(earlier);
        demand.input_tokens = demand.input_tokens.saturating_add(line_in);
        demand.max_line_input_tokens = demand.max_line_input_tokens.max(line_in);
        if asks_for_one_hour_cache(&req) {
            demand.one_hour_cache = true;
        }
        if let Request::Generate(g) = &req {
            match g.sampling.max_output_tokens {
                Some(m) => {
                    demand.explicit_lines = demand.explicit_lines.saturating_add(1);
                    demand.explicit_output_tokens =
                        demand.explicit_output_tokens.saturating_add(u64::from(m));
                }
                None => {
                    demand.unbounded_lines = demand.unbounded_lines.saturating_add(1);
                }
            }
        }
    }
    serde_json::to_value(demand).unwrap_or_default()
}

pub(super) fn evaluate(
    demand: Option<&Value>,
    file: &FileRow,
    kind: RequestKind,
    batch_prices: &Fields,
    provider: &ProviderRef,
) -> (WorstCase, Option<Unpriced>) {
    let parsed = demand
        .and_then(|v| serde_json::from_value::<BatchDemand>(v.clone()).ok())
        .unwrap_or_else(|| {
            let bytes = u64::try_from(file.bytes.max(0)).unwrap_or(0);
            let lines = u64::try_from(file.lines.max(1)).unwrap_or(1);
            let est_in = tokens_of_bytes(bytes);
            BatchDemand {
                input_tokens: est_in,
                max_line_input_tokens: est_in,
                explicit_output_tokens: 0,
                explicit_lines: 0,
                unbounded_lines: if kind == RequestKind::Embed { 0 } else { lines },
                unbounded_input: false,
                one_hour_cache: false,
                unpriced: None,
            }
        });
    let total_lines = parsed
        .explicit_lines
        .saturating_add(parsed.unbounded_lines)
        .max(u64::try_from(file.lines.max(1)).unwrap_or(1));
    let context = batch_prices.context_limit;
    let clamp_total = |tokens: u64, n: u64| match context {
        Some(c) => tokens.min(c.saturating_mul(n)),
        None => tokens,
    };
    let input = clamp_total(parsed.input_tokens, total_lines);
    let max_line_in = context.map_or(parsed.max_line_input_tokens, |c| {
        parsed.max_line_input_tokens.min(c)
    });
    let prices = cost::prices_for_prompt(batch_prices, max_line_in);
    let limit_holds = !(provider.reasoning_beyond_output_limit() && prices.reasoning == Some(true));
    let (explicit_out, unbounded_lines) = if limit_holds {
        (
            clamp_total(parsed.explicit_output_tokens, parsed.explicit_lines),
            parsed.unbounded_lines,
        )
    } else {
        (
            0,
            parsed.explicit_lines.saturating_add(parsed.unbounded_lines),
        )
    };
    let per_line_out = prices
        .output_limit
        .or(context)
        .map(|o| context.map_or(o, |c| o.min(c)));
    let (output, bounded) = if kind == RequestKind::Embed {
        (0, true)
    } else if unbounded_lines == 0 {
        (explicit_out, true)
    } else if let Some(per_line) = per_line_out {
        (
            explicit_out.saturating_add(unbounded_lines.saturating_mul(per_line)),
            true,
        )
    } else {
        (explicit_out, false)
    };
    let usage = Usage::new(input, output);
    let mut priced = prices.clone();
    priced.price_tiers = None;
    priced.output_price = [prices.output_price, prices.reasoning_price]
        .into_iter()
        .flatten()
        .max();
    let one_hour = parsed
        .one_hour_cache
        .then(|| cost::one_hour_cache_write_price(&prices))
        .flatten();
    priced.input_price = [
        prices.input_price,
        prices.cache_read_price,
        prices.cache_write_price,
        one_hour,
    ]
    .into_iter()
    .flatten()
    .max();
    let worst = if !bounded || parsed.unbounded_input {
        WorstCase {
            tokens: i64::MAX,
            nano_usd: i64::MAX,
            bounded: true,
        }
    } else {
        let total_tokens = cost::total_tokens(&usage);
        WorstCase {
            tokens: i64::try_from(total_tokens).unwrap_or(i64::MAX),
            nano_usd: cost::nano_usd(cost::cost(&usage, &priced)),
            bounded: true,
        }
    };
    let unpriced = parsed.unpriced.map(|u| Unpriced {
        param: static_param(&u.param),
        feature: u.feature,
    });
    (worst, unpriced)
}

pub(super) async fn admit_batch(
    state: &AppState,
    caller: &Caller,
    (batch_id, at): (Uuid, time::OffsetDateTime),
    (entry, eff, percent): (&Entry, &Effective, u8),
    file: &FileRow,
) -> Result<Reservation, KppError> {
    let endpoint = file.endpoint.as_deref().unwrap_or("/v1/chat/completions");
    let kind = ENDPOINTS
        .iter()
        .find(|(e, _, _)| *e == endpoint)
        .map_or(RequestKind::Generate, |(_, _, k)| *k);
    let batch_prices = cost::at_share(&eff.fields, percent);
    let (worst, line_unpriced) = evaluate(
        file.demand.as_ref(),
        file,
        kind,
        &batch_prices,
        &eff.provider,
    );
    let unpriced = cost::unpriced_model(&batch_prices, &entry.overrides, kind, &eff.model_id)
        .or(line_unpriced);
    let admission = Admission {
        request_id: RequestId::from_uuid(batch_id),
        caller: caller.clone(),
        catalogue_model_id: entry.id,
        kind,
        at,
        timeout: BATCH_HOLD,
        worst,
        unpriced,
        sends_audio: false,
    };
    state
        .limits
        .admit(state, &admission)
        .await
        .map_err(pipeline::limited)
}

pub(super) async fn release_batch(state: &AppState, row: &BatchRow) {
    let caller = Caller {
        org_id: row.org_id,
        project_id: row.project_id,
        key_id: row.api_key_id,
        user_id: row.user_id,
        service_account_id: row.service_account_id,
    };
    crate::services::limits::release_batch_hold(
        state,
        &caller,
        row.catalogue_model_id,
        RequestId::from_uuid(row.id),
        row.created_at,
        state.clock.now(),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn dummy_file(lines: i32, bytes: i64) -> FileRow {
        FileRow {
            id: Uuid::nil(),
            org_id: Uuid::nil(),
            project_id: Uuid::nil(),
            api_key_id: None,
            user_id: None,
            service_account_id: None,
            credential_id: Uuid::nil(),
            provider: "openai".into(),
            upstream_id: "file-1".into(),
            purpose: "batch".into(),
            filename: "in.jsonl".into(),
            bytes,
            lines,
            endpoint: Some("/v1/chat/completions".into()),
            catalogue_model_id: None,
            model: Some("gpt-4o".into()),
            upstream_model: Some("gpt-4o".into()),
            demand: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            expires_at: None,
        }
    }

    #[test]
    fn batch_demand_sums_capped_and_uncapped_lines_at_the_batch_price() {
        let openai = ProviderRef::from(kaeso_core::provider::provider("openai").unwrap());
        let prices = Fields {
            input_price: Some("1.0".parse().unwrap()),
            output_price: Some("2.0".parse().unwrap()),
            output_limit: Some(1_000),
            price_tiers: Some(vec![kaeso_core::catalogue::PriceTier {
                context_above: 200,
                input_price: Some("10.0".parse().unwrap()),
                output_price: Some("20.0".parse().unwrap()),
                cache_read_price: None,
                cache_write_price: None,
                cache_write_1h_price: None,
            }]),
            ..Fields::default()
        };
        let demand = serde_json::to_value(BatchDemand {
            input_tokens: 300,
            max_line_input_tokens: 60,
            explicit_output_tokens: 50,
            explicit_lines: 1,
            unbounded_lines: 1,
            unbounded_input: false,
            one_hour_cache: false,
            unpriced: Some(StoredUnpriced {
                param: "service_tier".into(),
                feature: "service_tier: priority".into(),
            }),
        })
        .unwrap();
        let (worst, unpriced) = evaluate(
            Some(&demand),
            &dummy_file(2, 400),
            RequestKind::Generate,
            &prices,
            &openai,
        );
        assert!(worst.bounded);
        assert_eq!(worst.tokens, 1_350);
        // 300 * $1/1M + 1_050 * $2/1M = $0.0024 = 2_400_000 nano-USD (not re-tiered to $10/$20).
        assert_eq!(worst.nano_usd, 2_400_000);
        assert_eq!(
            unpriced,
            Some(Unpriced {
                param: "service_tier",
                feature: "service_tier: priority".into(),
            })
        );

        let uncapped_model = Fields {
            input_price: Some("1.0".parse().unwrap()),
            output_price: Some("2.0".parse().unwrap()),
            ..Fields::default()
        };
        let (uncapped, _) = evaluate(
            Some(&demand),
            &dummy_file(2, 400),
            RequestKind::Generate,
            &uncapped_model,
            &openai,
        );
        assert!(uncapped.bounded);
        assert_eq!(uncapped.tokens, i64::MAX);
        assert_eq!(uncapped.nano_usd, i64::MAX);
    }
}
