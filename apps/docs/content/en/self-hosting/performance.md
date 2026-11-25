---
title: Performance
description: See how much time, CPU and memory Kaeso adds next to LiteLLM, Bifrost and Portkey on BerriAI's benchmark.
order: 14
---

Kaeso 0.7.3 ran on [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench), the benchmark from BerriAI, the company behind LiteLLM. LiteLLM, Bifrost and Portkey ran beside it on the same machine. Use the numbers to size a server and to compare Kaeso with the gateway you run now. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) draws them as charts and keeps every run, each with its scripts and raw results.

Every number is the median of three rounds. The benchmark's mock provider stands in for the model provider, so the numbers show the gateway's own cost, not a model's.

## Capacity

These numbers come from 64 clients against a mock that answers at once, with each gateway on the same two dedicated cores. The same driver reached about 19,600 requests a second straight to the mock, so each gateway ran at its own limit:

| Gateway | Requests/s | Latency p50 / p99 | CPU per request | Peak memory |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.3 | 4,835 | 12.8 / 22.6 ms | 412 µs | 181 + 633 MB |
| Kaeso 0.7.3, request logs on | 3,914 | 15.9 / 28.3 ms | 506 µs | 179 + 1,476 MB |
| LiteLLM Rust, main at `d4e8f96` | 2,068 | 30.4 / 53.7 ms | 842 µs | 86 MB |
| Bifrost 2.6.0 | 2,088 | 27.9 / 68.7 ms | 954 µs | 422 MB |
| Portkey 1.15.2 | 599 | 104.1 / 159.8 ms | 1,908 µs | 290 MB |
| LiteLLM Python 1.110.2 | 39 | 1,622.0 / 1,827.0 ms | 25,484 µs | 742 MB |

CPU per request is the gateway's own. Kaeso's memory is the gateway's plus its databases': PostgreSQL and Valkey, and ClickHouse when request logs are on. The other gateways ran without a database. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) shows each round's range.

## Added latency

Added latency is a request's time through the gateway minus the same request's time straight to the mock. The mock answers in about 22 ms, as a fast provider would. Each cell is p50 / p99:

| Gateway | 1 client | 4 clients | 16 clients |
| --- | --- | --- | --- |
| Kaeso 0.7.3 | +0.81 / +1.97 ms | +0.67 / +2.16 ms | +0.53 / +1.81 ms |
| Kaeso 0.7.3, request logs on | +0.83 / +2.44 ms | +0.68 / +2.39 ms | +0.55 / +1.93 ms |
| LiteLLM Rust, main at `d4e8f96` | +1.03 / +1.32 ms | +0.78 / +1.95 ms | +0.88 / +4.83 ms |
| Bifrost 2.6.0 | +1.28 / +1.79 ms | +1.02 / +2.60 ms | +1.01 / +4.10 ms |
| Portkey 1.15.2 | +2.32 / +6.63 ms | +1.44 / +9.36 ms | +5.9 / +30.1 ms |
| LiteLLM Python 1.110.2 | +26.5 / +29.7 ms | +87 / +110 ms | +376 / +420 ms |

There is no row for 64 clients. At that load the mock and the drivers share two cores and top out near 2,900 requests a second. Subtracting the direct run would then understate every gateway's time; the capacity table measures that load instead.

## Streams and agent sessions

The streaming run sends 200 streamed requests at 16 clients. The session runs replay a recorded Claude Code session and a recorded Codex session, 30 turns each. A session's added time is its whole time through the gateway minus straight to the mock:

| Gateway | Added time to first token | Chunk gap p99 | Claude Code session | Codex session |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.3 | +1.21 ms | 8.7 ms | +63.4 ms | +65.6 ms |
| Kaeso 0.7.3, request logs on | +1.49 ms | 9.5 ms | +71.1 ms | +68.8 ms |
| LiteLLM Rust, main at `d4e8f96` | +1.27 ms | 40.1 ms | +64.4 ms | +54.4 ms |
| Bifrost 2.6.0 | Not comparable | 20.2 ms | +183.9 ms | +118.8 ms |
| Portkey 1.15.2 | +28.19 ms | 9.3 ms | +114.2 ms | +120.5 ms |
| LiteLLM Python 1.110.2 | +402.27 ms | 71.2 ms | +814.8 ms | +835.7 ms |

Straight to the mock, the chunk gap is 8.3 ms at p99. Bifrost sends its first event before the provider answers, and the benchmark counts that as the first token, so its time to first token reads as negative. Every streamed session gave valid tool calls through every gateway.

## Where other gateways are ahead

- **Memory:** LiteLLM Rust never went above 87 MB and needs no database. Kaeso reached 181 MB, and its PostgreSQL and Valkey took another 633 MB.
- **One client:** with one request at a time, LiteLLM Rust's p99 was +1.32 ms against Kaeso's +1.97 ms, and no round of Kaeso's came closer.
- **A draw:** time to first token and the Claude Code session fall within each other's range.
- **Request logs:** with every request logged to ClickHouse, Kaeso served 19% fewer requests a second, and its databases took 1,476 MB.

## Since Kaeso 0.7.2

Kaeso 0.7.2 ran again on this run's machine right after 0.7.3, with and without request logs, by the same scripts. So the two versions compare without the machine in between. A difference counts only where the two sets of rounds don't overlap, and a value measured once, such as CPU or peak memory, only where it moved by more than a tenth:

| Measure | 0.7.2 | 0.7.3 | Reading |
| --- | --- | --- | --- |
| Requests a second at full load | 4,678 | 4,835 | Higher, beyond both sets of rounds |
| The same, request logs on | 3,849 | 3,914 | The same: the rounds overlap |
| CPU per request at full load | 427 µs | 412 µs | The same: within a tenth |
| Added at the median, 1 client | +0.76 ms | +0.81 ms | Higher, beyond both sets of rounds |
| Added at the median, 4 clients | +0.55 ms | +0.67 ms | Higher, beyond both sets of rounds |
| Added at the median, 16 clients | +0.56 ms | +0.53 ms | The same: the rounds overlap |
| Added at p99, 16 clients | +1.54 ms | +1.81 ms | The same: the rounds overlap |
| Chunk gap p99 | 8.7 ms | 8.7 ms | The same: the rounds overlap |
| Peak memory at full load, request logs on | 168 MB | 179 MB | The same: within a tenth |
| Peak memory of Kaeso's databases at full load | 567 MB | 633 MB | Higher, by more than a tenth |

The two versions ran one after the other, not interleaved. One change of 0.7.3 is on this benchmark's path: before a reply goes out, the gateway now notes the request so that a [source](../concepts/sources.md)'s copy of it counts once, which for an organization without sources is a cached check in a task of its own. Whether that or the machine over the hour between the two explains differences of 3% in capacity and 0.05 to 0.12 ms at the median, the rounds can't tell. The earlier runs, [0.7.2](https://kaeso.app/benchmarks/0.7.2) and [0.7.0](https://kaeso.app/benchmarks/0.7.0), ran on other machines of the same type and stay published with their scripts and raw results.

## What each gateway did

Every gateway ran as its project ships it, against the mock in the Anthropic Messages format:

| Gateway | Checked on each request | Kept |
| --- | --- | --- |
| Kaeso 0.7.3 | A project's [API key](../console/api-keys.md) and the models the project allows | Usage in PostgreSQL; no request logs |
| Kaeso 0.7.3, request logs on | The same | Usage in PostgreSQL, and every request in ClickHouse |
| LiteLLM Rust, `main` at `d4e8f96`, unreleased | One master key from its config | Nothing; no database |
| LiteLLM Python 1.110.2 | One master key from its config | Nothing; no database |
| Bifrost 2.6.0 | No key | Nothing; its logging and stores off |
| Portkey 1.15.2 | No key | Nothing; its logging off |

## How it ran

- **Machine:** a DigitalOcean CPU-Optimized c-4 with 4 dedicated vCPUs (Intel Xeon Platinum 8168, 2.7 GHz), 8 GB of memory, Ubuntu 24.04 and Docker, on 25 November 2026. c-4 machines were no longer offered in the region of the earlier runs, so this one is in another; the same driver reached over a quarter fewer requests a second straight to the mock there, and every gateway's numbers are lower.
- **Cores:** each gateway ran in Docker on the host network, pinned to two cores. The mock, the load drivers and Kaeso's databases ran on the other two.
- **Benchmark:** `BerriAI/ai-gateway-bench` at `44365ca`, with its mock provider and drivers unchanged: `load-probe` for requests, `run_http_bench.py` for streams and `session_runner.py` for sessions.
- **Mock provider:** its first token after 20 ms, then 40 tokens 5 ms apart. In the capacity run, it answers at once.
- **Load:** 1, 4, 16 and 64 clients sending 1,000, 2,000, 4,000 and 16,000 requests a round.
- **Rounds:** three, with the gateways in a new random order each round. Each gateway was measured right after a direct run of the same shape.
- **CPU and memory:** every process of the gateway's container, read four times a second, and the same for its databases.

The images ran by these digests:

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.3@sha256:e070906766362fc298babb51d2d0ee9a60deb07cbf30aac2e408f6d193f91553
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
curl -fsSLO https://kaeso.app/benchmarks/runs/0.7.3/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
# Kaeso 0.7.2 again, for the section above
ai-gateway-bench/.venv/bin/python run072.py kaeso kaeso-logs
ai-gateway-bench/.venv/bin/python cap072.py kaeso kaeso-logs
```

Three rounds of every gateway take about 90 minutes, and the capacity run another 30. The results land in `/root/out` and `/root/cap`, and 0.7.2's in `/root/out072` and `/root/cap072`. Absolute numbers change from machine to machine, so compare gateways measured in the same run.

## Next steps

- [Requirements](requirements.md): Size the server for the traffic you expect.
- [Several gateways](scaling.md): Run more gateways when one isn't enough.
- [Gateway benchmark](../development/benchmark.md): Measure Kaeso alone with streams, disconnects and slow apps.
