---
title: Performance
description: See how much time, CPU and memory Kaeso adds next to LiteLLM, Bifrost and Portkey on BerriAI's benchmark.
order: 14
---

Kaeso 0.7.4 ran on [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench), the benchmark from BerriAI, the company behind LiteLLM. LiteLLM, Bifrost and Portkey ran beside it on the same machine, and so did every earlier 0.7 version of Kaeso. Use the numbers to size a server and to compare Kaeso with the gateway you run now. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) draws them as charts and keeps every run, each with its scripts and raw results.

Every number is the median of three rounds. The benchmark's mock provider stands in for the model provider, so the numbers show the gateway's own cost, not a model's.

## Capacity

These numbers come from 64 clients against a mock that answers at once, with each gateway on the same two dedicated cores. The same driver reached about 23,200 requests a second straight to the mock, so each gateway ran at its own limit:

| Gateway | Requests/s | Latency p50 / p99 | CPU per request | Peak memory |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | 4,551 | 13.8 / 21.6 ms | 437 µs | 163 + 594 MB |
| Kaeso 0.7.4, request logs on | 3,962 | 15.8 / 27.2 ms | 500 µs | 188 + 1,489 MB |
| LiteLLM Rust, main at `d4e8f96` | 2,955 | 21.4 / 35.5 ms | 642 µs | 87 MB |
| Bifrost 2.6.0 | 2,242 | 24.7 / 63.9 ms | 887 µs | 414 MB |
| Portkey 1.15.2 | 619 | 101.2 / 148.6 ms | 1,829 µs | 303 MB |
| LiteLLM Python 1.110.2 | 55 | 1,162.0 / 1,331.9 ms | 18,286 µs | 742 MB |

CPU per request is the gateway's own. Kaeso's memory is the gateway's plus its databases': PostgreSQL and Valkey, and ClickHouse when request logs are on. The other gateways ran without a database. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) shows each round's range.

## Added latency

Added latency is a request's time through the gateway minus the same request's time straight to the mock. The mock answers in about 22 ms, as a fast provider would. Each cell is p50 / p99:

| Gateway | 1 client | 4 clients | 16 clients |
| --- | --- | --- | --- |
| Kaeso 0.7.4 | +0.94 / +1.96 ms | +0.92 / +1.81 ms | +0.64 / +1.86 ms |
| Kaeso 0.7.4, request logs on | +0.91 / +2.24 ms | +0.99 / +2.53 ms | +0.77 / +2.08 ms |
| LiteLLM Rust, main at `d4e8f96` | +1.05 / +1.16 ms | +0.97 / +1.67 ms | +0.82 / +2.43 ms |
| Bifrost 2.6.0 | +1.39 / +1.75 ms | +1.31 / +2.76 ms | +0.89 / +5.03 ms |
| Portkey 1.15.2 | +2.63 / +6.76 ms | +2.35 / +9.35 ms | +5.1 / +24.4 ms |
| LiteLLM Python 1.110.2 | +20.4 / +22.2 ms | +65.2 / +76.7 ms | +260 / +284 ms |

There is no row for 64 clients. At that load the mock and the drivers share two cores and top out near 2,900 requests a second. Subtracting the direct run would then understate every gateway's time; the capacity table measures that load instead.

## Streams and agent sessions

The streaming run sends 200 streamed requests at 16 clients. The session runs replay a recorded Claude Code session and a recorded Codex session, 30 turns each. A session's added time is its whole time through the gateway minus straight to the mock:

| Gateway | Added time to first token | Chunk gap p99 | Claude Code session | Codex session |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | +1.08 ms | 8.0 ms | +50.7 ms | +52.1 ms |
| Kaeso 0.7.4, request logs on | +1.11 ms | 9.0 ms | +60.2 ms | +80.7 ms |
| LiteLLM Rust, main at `d4e8f96` | +1.32 ms | 40.3 ms | +54.3 ms | +59.3 ms |
| Bifrost 2.6.0 | Not comparable | 20.4 ms | +168.8 ms | +104.8 ms |
| Portkey 1.15.2 | +28.20 ms | 9.8 ms | +121.0 ms | +125.2 ms |
| LiteLLM Python 1.110.2 | +227.63 ms | 50.5 ms | +642.6 ms | +647.0 ms |

Straight to the mock, the chunk gap is 7.7 ms at p99. Bifrost sends its first event before the provider answers, and the benchmark counts that as the first token, so its time to first token reads as negative. Every streamed session gave valid tool calls through every gateway.

## Where other gateways are ahead

- **Memory:** LiteLLM Rust never went above 87 MB and needs no database. Kaeso reached 163 MB, and its PostgreSQL and Valkey took another 594 MB.
- **One client:** with one request at a time, LiteLLM Rust's p99 was +1.16 ms against Kaeso's +1.96 ms, and no round of Kaeso's came closer.
- **Request logs:** with every request logged to ClickHouse, Kaeso served 13% fewer requests a second, and its databases took 1,489 MB.

## Every 0.7 version on one machine

Kaeso 0.7.0, 0.7.1, 0.7.2, 0.7.3 and 0.7.4 each ran as a gateway of its own in this run, in the same shuffled rounds as the other gateways, so the versions compare number for number, without the machine or the hour in between. A step counts only where two versions' rounds don't overlap, and a value measured once, such as CPU or peak memory, only where it moved by more than a tenth:

| Version | Requests/s | Request logs on | Added p50, 1 / 4 / 16 clients | Added p99, 16 clients | CPU per request | Peak memory |
| --- | --- | --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | 4,551 | 3,962 | +0.94 / +0.92 / +0.64 ms | +1.86 ms | 437 µs | 163 + 594 MB |
| Kaeso 0.7.3 | 5,204 | 4,371 | +1.02 / +1.06 / +0.73 ms | +1.67 ms | 384 µs | 102 + 543 MB |
| Kaeso 0.7.2 | 4,666 | 4,032 | +0.92 / +0.90 / +0.66 ms | +1.70 ms | 428 µs | 178 + 579 MB |
| Kaeso 0.7.1 | 4,413 | 3,781 | +0.96 / +0.99 / +0.83 ms | +1.90 ms | 453 µs | 110 + 565 MB |
| Kaeso 0.7.0 | 4,748 | 3,891 | +0.98 / +0.99 / +0.84 ms | +2.03 ms | 421 µs | 113 + 283 MB |

- **0.7.3 to 0.7.4:** 0.7.4 takes back the wait 0.7.3 added to every reply, and the median it adds at 1 and 4 clients is below 0.7.3's again. 0.7.3 served the most requests of any version in every round; 0.7.4 served 13% fewer, with 14% more CPU per request, which puts it within the rounds of 0.7.2. Its peak memory at full load rose from 102 to 163 MB.
- **0.7.2 to 0.7.3:** more requests a second and less CPU per request, a higher median at 1, 4 and 16 clients, and peak memory down from 178 to 102 MB.
- **0.7.1 to 0.7.2:** more requests a second, with and without request logs, a lower median at 1 and 4 clients, and peak memory up from 110 to 178 MB.
- **0.7.0 to 0.7.1:** 7% fewer requests a second, and the databases' peak memory doubled; it stayed there in every later version.

The rounds show each change; they don't show its cause. The earlier runs, [0.7.3](https://kaeso.app/benchmarks/0.7.3), [0.7.2](https://kaeso.app/benchmarks/0.7.2) and [0.7.0](https://kaeso.app/benchmarks/0.7.0), ran on other machines of the same type and stay published with their scripts and raw results.
## What each gateway did

Every gateway ran as its project ships it, against the mock in the Anthropic Messages format:

| Gateway | Checked on each request | Kept |
| --- | --- | --- |
| Kaeso 0.7.4 | A project's [API key](../console/api-keys.md) and the models the project allows | Usage in PostgreSQL; no request logs |
| Kaeso 0.7.4, request logs on | The same | Usage in PostgreSQL, and every request in ClickHouse |
| LiteLLM Rust, `main` at `d4e8f96`, unreleased | One master key from its config | Nothing; no database |
| LiteLLM Python 1.110.2 | One master key from its config | Nothing; no database |
| Bifrost 2.6.0 | No key | Nothing; its logging and stores off |
| Portkey 1.15.2 | No key | Nothing; its logging off |

## How it ran

- **Machine:** a DigitalOcean CPU-Optimized c-4 with 4 dedicated vCPUs (Intel Xeon Platinum 8280, 2.7 GHz), 8 GB of memory, Ubuntu 24.04 and Docker, on 25 and 26 November 2026. Before the run, its scheduled jobs, package updates, snapd and the provider's agents were turned off, and so were transparent huge pages and swapping. The same driver reached about 23,200 requests a second straight to the mock there, where the machines of the 0.7.0 and 0.7.2 runs reached about 28,000, so every gateway's numbers are lower than in those runs.
- **Cores:** each gateway ran in Docker on the host network, pinned to two cores. The mock, the load drivers and Kaeso's databases ran on the other two.
- **Benchmark:** `BerriAI/ai-gateway-bench` at `44365ca`, with its mock provider and drivers unchanged: `load-probe` for requests, `run_http_bench.py` for streams and `session_runner.py` for sessions.
- **Mock provider:** its first token after 20 ms, then 40 tokens 5 ms apart. In the capacity run, it answers at once.
- **Load:** 1, 4, 16 and 64 clients sending 1,000, 2,000, 4,000 and 16,000 requests a round.
- **Rounds:** three, with the gateways in a new random order each round. Each gateway was measured right after a direct run of the same shape.
- **CPU and memory:** every process of the gateway's container, read four times a second, and the same for its databases.

The images ran by these digests:

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.4@sha256:25fa94e10da01773b0823398ba72c2e1faec3521f73ea9725db877b615be83b3
ghcr.io/logicplanes/kaeso-gateway:0.7.3@sha256:e070906766362fc298babb51d2d0ee9a60deb07cbf30aac2e408f6d193f91553
ghcr.io/logicplanes/kaeso-gateway:0.7.2@sha256:7db4ee52773bc546746fb5e3d513457929d1782fdb89795f7b9c9f406a94ae79
ghcr.io/logicplanes/kaeso-gateway:0.7.1@sha256:42f7e818b43e9422a432747a9841df8952819de4c8e557f5dae8216f76409e7e
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
curl -fsSLO https://kaeso.app/benchmarks/runs/0.7.4/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
```

Three rounds of every gateway take about two hours, and the capacity run another 20 minutes. The results land in `/root/out` and `/root/cap`. Each Kaeso version is a gateway of its own, `kaeso-0.7.4` and `kaeso-logs-0.7.4` and so on; name gateways after `run.py` to run only those. Absolute numbers change from machine to machine, so compare gateways measured in the same run.

## Next steps

- [Requirements](requirements.md): Size the server for the traffic you expect.
- [Several gateways](scaling.md): Run more gateways when one isn't enough.
- [Gateway benchmark](../development/benchmark.md): Measure Kaeso alone with streams, disconnects and slow apps.
