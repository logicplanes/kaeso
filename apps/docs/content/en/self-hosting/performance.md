---
title: Performance
description: See how much time, CPU and memory Kaeso adds next to LiteLLM, Bifrost and Portkey on BerriAI's benchmark.
order: 14
---

Kaeso 0.7.2 ran on [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench), the benchmark from BerriAI, the company behind LiteLLM. LiteLLM, Bifrost and Portkey ran beside it on the same machine. Use the numbers to size a server and to compare Kaeso with the gateway you run now. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) draws them as charts and offers the scripts and every raw result.

Every number is the median of three rounds. The benchmark's mock provider stands in for the model provider, so the numbers show the gateway's own cost, not a model's.

## Capacity

These numbers come from 64 clients against a mock that answers at once, with each gateway on the same two dedicated cores. The same driver reached about 28,000 requests a second straight to the mock, so each gateway ran at its own limit:

| Gateway | Requests/s | Latency p50 / p99 | CPU per request | Peak memory |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.2 | 6,243 | 10.2 / 15.9 ms | 320 µs | 104 + 559 MB |
| Kaeso 0.7.2, request logs on | 5,177 | 12.3 / 20.2 ms | 385 µs | 212 + 1,453 MB |
| LiteLLM Rust, main at `d4e8f96` | 3,286 | 19.3 / 31.9 ms | 560 µs | 86 MB |
| Bifrost 2.6.0 | 2,858 | 20.6 / 52.3 ms | 693 µs | 452 MB |
| Portkey 1.15.2 | 821 | 77.9 / 113.5 ms | 1,366 µs | 301 MB |
| LiteLLM Python 1.110.2 | 75 | 849.0 / 918.0 ms | 13,381 µs | 744 MB |

CPU per request is the gateway's own. Kaeso's memory is the gateway's plus its databases': PostgreSQL and Valkey, and ClickHouse when request logs are on. The other gateways ran without a database. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) shows each round's range.

## Added latency

Added latency is a request's time through the gateway minus the same request's time straight to the mock. The mock answers in about 22 ms, as a fast provider would. Each cell is p50 / p99:

| Gateway | 1 client | 4 clients | 16 clients |
| --- | --- | --- | --- |
| Kaeso 0.7.2 | +0.87 / +1.63 ms | +0.66 / +1.23 ms | +0.79 / +1.73 ms |
| Kaeso 0.7.2, request logs on | +0.67 / +1.62 ms | +0.52 / +1.97 ms | +0.56 / +1.83 ms |
| LiteLLM Rust, main at `d4e8f96` | +1.01 / +1.09 ms | +0.89 / +2.11 ms | +0.67 / +2.33 ms |
| Bifrost 2.6.0 | +1.09 / +1.24 ms | +0.96 / +2.36 ms | +0.57 / +2.98 ms |
| Portkey 1.15.2 | +2.12 / +4.79 ms | +1.11 / +6.49 ms | +2.9 / +16.4 ms |
| LiteLLM Python 1.110.2 | +15.8 / +17.3 ms | +49.3 / +57.8 ms | +185 / +206 ms |

There is no row for 64 clients. At that load the mock and the drivers share two cores and top out near 2,900 requests a second. Subtracting the direct run would then understate every gateway's time; the capacity table measures that load instead.

## Streams and agent sessions

The streaming run sends 200 streamed requests at 16 clients. The session runs replay a recorded Claude Code session and a recorded Codex session, 30 turns each. A session's added time is its whole time through the gateway minus straight to the mock:

| Gateway | Added time to first token | Chunk gap p99 | Claude Code session | Codex session |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.2 | +0.95 ms | 7.7 ms | +43.5 ms | +44.8 ms |
| Kaeso 0.7.2, request logs on | +0.98 ms | 8.0 ms | +42.1 ms | +58.6 ms |
| LiteLLM Rust, main at `d4e8f96` | +0.94 ms | 40.4 ms | +45.2 ms | +46.6 ms |
| Bifrost 2.6.0 | Not comparable | 20.6 ms | +125.3 ms | +94.5 ms |
| Portkey 1.15.2 | +27.54 ms | 8.1 ms | +100.0 ms | +104.5 ms |
| LiteLLM Python 1.110.2 | +114.17 ms | 33.3 ms | +510.5 ms | +504.6 ms |

Straight to the mock, the chunk gap is 7.5 ms at p99. Bifrost sends its first event before the provider answers, and the benchmark counts that as the first token, so its time to first token reads as negative. Every streamed session gave valid tool calls through every gateway.

## Where other gateways are ahead

- **Memory:** LiteLLM Rust never went above 86 MB and needs no database. Kaeso reached 160 MB, and its PostgreSQL and Valkey took another 559 MB.
- **Sixteen clients:** at the median, Bifrost added +0.57 ms and LiteLLM Rust +0.67 ms, against Kaeso's +0.79 ms, and every round of Bifrost's came out lower than every round of Kaeso's. At p99, Kaeso added the least.
- **A draw:** time to first token and the Claude Code session fall within each other's range.
- **Request logs:** with every request logged to ClickHouse, Kaeso served 17% fewer requests a second, and its databases took 1,453 MB.

## Since Kaeso 0.7.0

The run of 22 November 2026 measured Kaeso 0.7.0 with the same benchmark, scripts and images of every other gateway, on another c-4 of the same type. The unchanged gateways' capacity moved by up to 5% between the two runs, so a difference counts only where both runs' rounds don't overlap and the other gateways didn't move the same way:

| Measure | 0.7.0 | 0.7.2 | Reading |
| --- | --- | --- | --- |
| Requests a second at full load | 6,338 | 6,243 | The same: the rounds overlap |
| The same, request logs on | 5,324 | 5,177 | The same: the rounds overlap |
| Added at the median, 1 client | +0.73 ms | +0.87 ms | Higher, but LiteLLM Rust rose about as much: the machine |
| Added at the median, 4 clients | +0.56 ms | +0.66 ms | Higher, but LiteLLM Rust and Bifrost rose about as much: the machine |
| Added at the median, 16 clients | +0.54 ms | +0.79 ms | Higher, beyond both runs' rounds |
| Added at p99, 16 clients | +1.47 ms | +1.73 ms | The same: the rounds overlap |
| Chunk gap p99 | 7.8 ms | 7.7 ms | The same: the rounds overlap |
| Peak memory at full load, request logs on | 450 MB | 212 MB | Lower |
| Peak memory of Kaeso's databases at full load | 271 MB | 559 MB | Higher |

0.7.2 keeps the request log rows that wait for ClickHouse within [`logs.buffer_size`](configuration.md), which fits the lower peak memory with request logs on. Kaeso's PostgreSQL peaked at about twice the memory while its CPU per request stayed the same, and Kaeso's added median at 16 clients rose while the other gateways' didn't. The runs don't show whether 0.7.2 or the machine caused those two. The 0.7.0 run's [scripts](https://kaeso.app/benchmarks/2026-11-22/harness.tar.gz) and [raw results](https://kaeso.app/benchmarks/2026-11-22/raw.tar.gz) stay published.

## What each gateway did

Every gateway ran as its project ships it, against the mock in the Anthropic Messages format:

| Gateway | Checked on each request | Kept |
| --- | --- | --- |
| Kaeso 0.7.2 | A project's [API key](../console/api-keys.md) and the models the project allows | Usage in PostgreSQL; no request logs |
| Kaeso 0.7.2, request logs on | The same | Usage in PostgreSQL, and every request in ClickHouse |
| LiteLLM Rust, `main` at `d4e8f96`, unreleased | One master key from its config | Nothing; no database |
| LiteLLM Python 1.110.2 | One master key from its config | Nothing; no database |
| Bifrost 2.6.0 | No key | Nothing; its logging and stores off |
| Portkey 1.15.2 | No key | Nothing; its logging off |

## How it ran

- **Machine:** a DigitalOcean CPU-Optimized c-4 with 4 dedicated vCPUs (Intel Xeon Platinum 8358, 2.6 GHz), 8 GB of memory, Ubuntu 24.04 and Docker, on 25 November 2026.
- **Cores:** each gateway ran in Docker on the host network, pinned to two cores. The mock, the load drivers and Kaeso's databases ran on the other two.
- **Benchmark:** `BerriAI/ai-gateway-bench` at `44365ca`, with its mock provider and drivers unchanged: `load-probe` for requests, `run_http_bench.py` for streams and `session_runner.py` for sessions.
- **Mock provider:** its first token after 20 ms, then 40 tokens 5 ms apart. In the capacity run, it answers at once.
- **Load:** 1, 4, 16 and 64 clients sending 1,000, 2,000, 4,000 and 16,000 requests a round.
- **Rounds:** three, with the gateways in a new random order each round. Each gateway was measured right after a direct run of the same shape.
- **CPU and memory:** every process of the gateway's container, read four times a second, and the same for its databases.

The images ran by these digests:

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.2@sha256:7db4ee52773bc546746fb5e3d513457929d1782fdb89795f7b9c9f406a94ae79
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
curl -fsSLO https://kaeso.app/benchmarks/2026-11-25/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
```

Three rounds of every gateway take about 90 minutes, and the capacity run another 30. The results land in `/root/out` and `/root/cap`. Absolute numbers change from machine to machine, so compare gateways measured in the same run.

## Next steps

- [Requirements](requirements.md): Size the server for the traffic you expect.
- [Several gateways](scaling.md): Run more gateways when one isn't enough.
- [Gateway benchmark](../development/benchmark.md): Measure Kaeso alone with streams, disconnects and slow apps.
