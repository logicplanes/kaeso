---
title: Performance
description: See how much time, CPU and memory Kaeso adds next to LiteLLM, Bifrost and Portkey on BerriAI's benchmark.
order: 14
---

Kaeso 0.7.0 ran on [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench), the benchmark from BerriAI, the company behind LiteLLM. LiteLLM, Bifrost and Portkey ran beside it on the same machine. Use the numbers to size a server and to compare Kaeso with the gateway you run now. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) draws them as charts and offers the scripts and every raw result.

Every number is the median of three rounds. The benchmark's mock provider stands in for the model provider, so the numbers show the gateway's own cost, not a model's.

## Capacity

These numbers come from 64 clients against a mock that answers at once, with each gateway on the same two dedicated cores. The same driver reached about 28,000 requests a second straight to the mock, so each gateway ran at its own limit:

| Gateway | Requests/s | Latency p50 / p99 | CPU per request | Peak memory |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.0 | 6,338 | 10.0 / 15.6 ms | 314 µs | 101 + 271 MB |
| Kaeso 0.7.0, request logs on | 5,324 | 11.8 / 20.6 ms | 373 µs | 450 + 1,163 MB |
| LiteLLM Rust, main at `d4e8f96` | 3,211 | 19.6 / 33.2 ms | 573 µs | 86 MB |
| Bifrost 2.6.0 | 2,806 | 21.0 / 54.6 ms | 708 µs | 405 MB |
| Portkey 1.15.2 | 779 | 79.9 / 118.3 ms | 1,449 µs | 293 MB |
| LiteLLM Python 1.110.2 | 74 | 858.1 / 974.0 ms | 13,563 µs | 743 MB |

CPU per request is the gateway's own. Kaeso's memory is the gateway's plus its databases': PostgreSQL and Valkey, and ClickHouse when request logs are on. The other gateways ran without a database. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) shows each round's range.

## Added latency

Added latency is a request's time through the gateway minus the same request's time straight to the mock. The mock answers in about 22 ms, as a fast provider would. Each cell is p50 / p99:

| Gateway | 1 client | 4 clients | 16 clients |
| --- | --- | --- | --- |
| Kaeso 0.7.0 | +0.73 / +1.66 ms | +0.56 / +1.15 ms | +0.54 / +1.47 ms |
| Kaeso 0.7.0, request logs on | +0.77 / +2.22 ms | +0.60 / +1.90 ms | +0.62 / +1.45 ms |
| LiteLLM Rust, main at `d4e8f96` | +0.87 / +0.98 ms | +0.76 / +1.68 ms | +0.69 / +2.26 ms |
| Bifrost 2.6.0 | +1.10 / +1.32 ms | +0.85 / +2.41 ms | +0.71 / +4.08 ms |
| Portkey 1.15.2 | +2.04 / +5.26 ms | +1.17 / +6.99 ms | +3.1 / +20.4 ms |
| LiteLLM Python 1.110.2 | +16.0 / +17.8 ms | +50.9 / +60.5 ms | +188 / +216 ms |

There is no row for 64 clients. At that load the mock and the drivers share two cores and top out near 2,900 requests a second. Subtracting the direct run would then understate every gateway's time; the capacity table measures that load instead.

## Streams and agent sessions

The streaming run sends 200 streamed requests at 16 clients. The session runs replay a recorded Claude Code session and a recorded Codex session, 30 turns each. A session's added time is its whole time through the gateway minus straight to the mock:

| Gateway | Added time to first token | Chunk gap p99 | Claude Code session | Codex session |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.0 | +0.87 ms | 7.8 ms | +41.5 ms | +34.0 ms |
| Kaeso 0.7.0, request logs on | +0.94 ms | 8.1 ms | +66.0 ms | +65.5 ms |
| LiteLLM Rust, main at `d4e8f96` | +0.96 ms | 40.4 ms | +41.0 ms | +44.2 ms |
| Bifrost 2.6.0 | Not comparable | 20.5 ms | +126.4 ms | +91.3 ms |
| Portkey 1.15.2 | +27.54 ms | 8.6 ms | +91.4 ms | +104.2 ms |
| LiteLLM Python 1.110.2 | +118.92 ms | 34.0 ms | +502.1 ms | +507.7 ms |

Straight to the mock, the chunk gap is 7.5 ms at p99. Bifrost sends its first event before the provider answers, and the benchmark counts that as the first token, so its time to first token reads as negative. Every streamed session gave valid tool calls through every gateway.

## Where other gateways are ahead

- **Memory:** LiteLLM Rust never went above 86 MB and needs no database. Kaeso reached 164 MB, and its PostgreSQL and Valkey took another 271 MB.
- **One client:** with one request at a time, LiteLLM Rust's p99 was +0.98 ms against Kaeso's +1.66 ms, and no round of Kaeso's came closer.
- **A draw:** time to first token and the Claude Code session fall within each other's range.
- **Request logs:** with every request logged to ClickHouse, Kaeso served 16% fewer requests a second, and its databases took 1,163 MB.

## What each gateway did

Every gateway ran as its project ships it, against the mock in the Anthropic Messages format:

| Gateway | Checked on each request | Kept |
| --- | --- | --- |
| Kaeso 0.7.0 | A project's [API key](../console/api-keys.md) and the models the project allows | Usage in PostgreSQL; no request logs |
| Kaeso 0.7.0, request logs on | The same | Usage in PostgreSQL, and every request in ClickHouse |
| LiteLLM Rust, `main` at `d4e8f96`, unreleased | One master key from its config | Nothing; no database |
| LiteLLM Python 1.110.2 | One master key from its config | Nothing; no database |
| Bifrost 2.6.0 | No key | Nothing; its logging and stores off |
| Portkey 1.15.2 | No key | Nothing; its logging off |

## How it ran

- **Machine:** a DigitalOcean CPU-Optimized c-4 with 4 dedicated vCPUs (Intel Xeon Platinum 8358, 2.6 GHz), 8 GB of memory, Ubuntu 24.04 and Docker, on 22 November 2026.
- **Cores:** each gateway ran in Docker on the host network, pinned to two cores. The mock, the load drivers and Kaeso's databases ran on the other two.
- **Benchmark:** `BerriAI/ai-gateway-bench` at `44365ca`, with its mock provider and drivers unchanged: `load-probe` for requests, `run_http_bench.py` for streams and `session_runner.py` for sessions.
- **Mock provider:** its first token after 20 ms, then 40 tokens 5 ms apart. In the capacity run, it answers at once.
- **Load:** 1, 4, 16 and 64 clients sending 1,000, 2,000, 4,000 and 16,000 requests a round.
- **Rounds:** three, with the gateways in a new random order each round. Each gateway was measured right after a direct run of the same shape.
- **CPU and memory:** every process of the gateway's container, read four times a second, and the same for its databases.

The images ran by these digests:

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.0@sha256:3d32fd7827417e49475b4ac9ad7d25f7daed28f02fa489ef4077ff501faaae1a
ghcr.io/berriai/litellm:v1.110.2@sha256:ef798594ca3db8d81bad2e1439f536f49b71fe095ee9f20416f5e149b4e22035
maximhq/bifrost:v2.6.0@sha256:5968d007ba4f74ee01bc4ceb071decb9927a0b7bb43dbb9072208f75e2d522e3
portkeyai/gateway:1.15.2@sha256:97f094d9c8a764cbfaa2a7138c0017b247ca923bb06db1b4c13b7f8a33b5200d
```

LiteLLM Rust has no released image yet. It was built from `BerriAI/litellm` at `d4e8f96` with `rust:1.98.0`.

> [!IMPORTANT]
> Kaeso's author ran this benchmark on one machine. BerriAI calls its own published results single-host and vendor-run, and the same holds here. Run it yourself before you rely on it.

## Run it yourself

Use a fresh Ubuntu 24.04 machine with four cores, as root. The setup installs Docker, builds the benchmark and LiteLLM Rust at the same commits, and pulls every image by its digest:

```bash
curl -fsSLO https://kaeso.app/benchmarks/2026-11-22/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
```

Three rounds of every gateway take about 90 minutes, and the capacity run another 30. The results land in `/root/out` and `/root/cap`.

## Next steps

- [Requirements](requirements.md): Size the server for the traffic you expect.
- [Several gateways](scaling.md): Run more gateways when one isn't enough.
- [Gateway benchmark](../development/benchmark.md): Measure Kaeso alone with streams, disconnects and slow apps.
