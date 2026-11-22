---
title: 性能
description: 了解在 BerriAI 的基准测试中，Kaeso 与 LiteLLM、Bifrost 和 Portkey 相比增加了多少时间、CPU 和内存。
order: 14
---

Kaeso 0.7.0 在 [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench) 上运行，这是 LiteLLM 背后的公司 BerriAI 发布的基准测试。LiteLLM、Bifrost 和 Portkey 在同一台机器上一同运行。您可以用这些数字来规划服务器，并将 Kaeso 与您现在运行的网关进行比较。[kaeso.app/benchmarks](https://kaeso.app/benchmarks) 以图表展示这些数字，并提供脚本和全部原始结果。

每个数字都是三轮的中位数。基准测试的模拟服务商代替了模型服务商，因此这些数字反映的是网关本身的开销，而不是模型的开销。

## 容量

这些数字来自 64 个客户端对一个立即应答的模拟服务商发起请求，每个网关都运行在相同的两个专用核心上。同一驱动程序直连模拟服务商约可达每秒 28,000 个请求，因此每个网关都运行在它自身的极限：

| 网关 | 每秒请求数 | 延迟 p50 / p99 | 每个请求的 CPU | 内存峰值 |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.0 | 6,338 | 10.0 / 15.6 ms | 314 µs | 101 + 271 MB |
| Kaeso 0.7.0，开启请求日志 | 5,324 | 11.8 / 20.6 ms | 373 µs | 450 + 1,163 MB |
| LiteLLM Rust，`main` 分支 `d4e8f96` | 3,211 | 19.6 / 33.2 ms | 573 µs | 86 MB |
| Bifrost 2.6.0 | 2,806 | 21.0 / 54.6 ms | 708 µs | 405 MB |
| Portkey 1.15.2 | 779 | 79.9 / 118.3 ms | 1,449 µs | 293 MB |
| LiteLLM Python 1.110.2 | 74 | 858.1 / 974.0 ms | 13,563 µs | 743 MB |

每个请求的 CPU 只计网关本身。Kaeso 的内存是网关加上其数据库的内存：PostgreSQL 和 Valkey，开启请求日志时还包括 ClickHouse。其他网关运行时没有数据库。[kaeso.app/benchmarks](https://kaeso.app/benchmarks) 展示每一轮的范围。

## 附加延迟

附加延迟是请求经过网关的时间减去同一请求直连模拟服务商的时间。模拟服务商约 22 ms 应答，与快速的服务商相当。每个单元格为 p50 / p99：

| 网关 | 1 个客户端 | 4 个客户端 | 16 个客户端 |
| --- | --- | --- | --- |
| Kaeso 0.7.0 | +0.73 / +1.66 ms | +0.56 / +1.15 ms | +0.54 / +1.47 ms |
| Kaeso 0.7.0，开启请求日志 | +0.77 / +2.22 ms | +0.60 / +1.90 ms | +0.62 / +1.45 ms |
| LiteLLM Rust，`main` 分支 `d4e8f96` | +0.87 / +0.98 ms | +0.76 / +1.68 ms | +0.69 / +2.26 ms |
| Bifrost 2.6.0 | +1.10 / +1.32 ms | +0.85 / +2.41 ms | +0.71 / +4.08 ms |
| Portkey 1.15.2 | +2.04 / +5.26 ms | +1.17 / +6.99 ms | +3.1 / +20.4 ms |
| LiteLLM Python 1.110.2 | +16.0 / +17.8 ms | +50.9 / +60.5 ms | +188 / +216 ms |

没有 64 个客户端的数据。在这一负载下，模拟服务商和驱动程序共用两个核心，上限约为每秒 2,900 个请求。此时减去直连运行会低估每个网关的耗时；这一负载改由容量表衡量。

## 流式输出与智能体会话

流式测试在 16 个客户端下发送 200 个流式请求。会话测试回放一个录制的 Claude Code 会话和一个录制的 Codex 会话，各 30 轮。会话的附加时间是经过网关的总时长减去直连的总时长：

| 网关 | 首个 token 附加时间 | 分块间隔 p99 | Claude Code 会话 | Codex 会话 |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.0 | +0.87 ms | 7.8 ms | +41.5 ms | +34.0 ms |
| Kaeso 0.7.0，开启请求日志 | +0.94 ms | 8.1 ms | +66.0 ms | +65.5 ms |
| LiteLLM Rust，`main` 分支 `d4e8f96` | +0.96 ms | 40.4 ms | +41.0 ms | +44.2 ms |
| Bifrost 2.6.0 | 不可比较 | 20.5 ms | +126.4 ms | +91.3 ms |
| Portkey 1.15.2 | +27.54 ms | 8.6 ms | +91.4 ms | +104.2 ms |
| LiteLLM Python 1.110.2 | +118.92 ms | 34.0 ms | +502.1 ms | +507.7 ms |

直连模拟服务商时，分块间隔的 p99 为 7.5 ms。Bifrost 在服务商应答之前就发送第一个事件，基准测试将其计为首个 token，因此它的首 token 时间为负数。每个流式会话经过每个网关都给出了有效的工具调用。

## 其他网关领先之处

- **内存：**LiteLLM Rust 从未超过 86 MB，且无需数据库。Kaeso 最高达到 164 MB，其 PostgreSQL 和 Valkey 另占 271 MB。
- **单个客户端：**每次只发一个请求时，LiteLLM Rust 的 p99 为 +0.98 ms，Kaeso 为 +1.66 ms，Kaeso 没有任何一轮更接近。
- **平手：**首个 token 时间和 Claude Code 会话都落在彼此的范围之内。
- **请求日志：**将每个请求记录到 ClickHouse 时，Kaeso 每秒处理的请求少了 16%，其数据库占用 1,163 MB。

## 每个网关做了什么

每个网关都按其项目的发布方式运行，对接使用 Anthropic Messages 格式的模拟服务商：

| 网关 | 每个请求检查的内容 | 保存的内容 |
| --- | --- | --- |
| Kaeso 0.7.0 | 项目的 [API 密钥](../console/api-keys.md)，以及项目允许的模型 | 用量存入 PostgreSQL；无请求日志 |
| Kaeso 0.7.0，开启请求日志 | 同上 | 用量存入 PostgreSQL，每个请求存入 ClickHouse |
| LiteLLM Rust，`main` 分支 `d4e8f96`，未发布 | 配置中的一个主密钥 | 不保存；无数据库 |
| LiteLLM Python 1.110.2 | 配置中的一个主密钥 | 不保存；无数据库 |
| Bifrost 2.6.0 | 不检查密钥 | 不保存；日志与存储均关闭 |
| Portkey 1.15.2 | 不检查密钥 | 不保存；日志关闭 |

## 测试方法

- **机器：**DigitalOcean CPU-Optimized c-4，4 个专用 vCPU（Intel Xeon Platinum 8358，2.6 GHz），8 GB 内存，Ubuntu 24.04 与 Docker，测试日期为 2026 年 11 月 22 日。
- **核心：**每个网关运行在 Docker 中，使用主机网络，固定在两个核心上。模拟服务商、负载驱动程序和 Kaeso 的数据库运行在另外两个核心上。
- **基准测试：**`BerriAI/ai-gateway-bench`，提交 `44365ca`，模拟服务商与驱动程序均未改动：`load-probe` 用于请求，`run_http_bench.py` 用于流式输出，`session_runner.py` 用于会话。
- **模拟服务商：**20 ms 后发出首个 token，随后以 5 ms 的间隔发出 40 个 token。容量测试中立即应答。
- **负载：**1、4、16 和 64 个客户端，每轮分别发送 1,000、2,000、4,000 和 16,000 个请求。
- **轮次：**三轮，每轮网关顺序重新随机。每个网关都在一次形态相同的直连运行之后立即测量。
- **CPU 与内存：**网关容器中的每个进程，每秒读取四次；其数据库同样读取。

各镜像按以下摘要运行：

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.0@sha256:3d32fd7827417e49475b4ac9ad7d25f7daed28f02fa489ef4077ff501faaae1a
ghcr.io/berriai/litellm:v1.110.2@sha256:ef798594ca3db8d81bad2e1439f536f49b71fe095ee9f20416f5e149b4e22035
maximhq/bifrost:v2.6.0@sha256:5968d007ba4f74ee01bc4ceb071decb9927a0b7bb43dbb9072208f75e2d522e3
portkeyai/gateway:1.15.2@sha256:97f094d9c8a764cbfaa2a7138c0017b247ca923bb06db1b4c13b7f8a33b5200d
```

LiteLLM Rust 尚无已发布的镜像。它由 `BerriAI/litellm` 的提交 `d4e8f96` 使用 `rust:1.98.0` 构建。

> [!IMPORTANT]
> 这次基准测试由 Kaeso 的作者在一台机器上完成。BerriAI 称其自己公布的结果为单机且由厂商测试，这里同样如此。在依赖这些结果之前，请自行复测。

## 自己运行

使用一台全新的四核 Ubuntu 24.04 机器，并以 root 身份运行。安装脚本会安装 Docker，按相同提交构建基准测试和 LiteLLM Rust，并按摘要拉取每个镜像：

```bash
curl -fsSLO https://kaeso.app/benchmarks/2026-11-22/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
```

所有网关跑完三轮约需 90 分钟，容量测试另需 30 分钟。结果保存在 `/root/out` 和 `/root/cap` 中。绝对数值因机器而异，请比较同一次运行中测得的网关。

## 下一步

- [系统要求](requirements.md)：按预期流量规划服务器。
- [多个网关](scaling.md)：一个网关不够时运行更多网关。
- [网关基准测试](../development/benchmark.md)：单独测量 Kaeso 在流式回复、断开连接和慢速应用下的表现。
