---
title: 性能
description: 了解在 BerriAI 的基准测试中，Kaeso 与 LiteLLM、Bifrost 和 Portkey 相比增加了多少时间、CPU 和内存。
order: 14
---

Kaeso 0.7.3 在 [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench) 上运行，这是 LiteLLM 背后的公司 BerriAI 发布的基准测试。LiteLLM、Bifrost 和 Portkey 在同一台机器上一同运行。您可以用这些数字来规划服务器，并将 Kaeso 与您现在运行的网关进行比较。[kaeso.app/benchmarks](https://kaeso.app/zh-CN/benchmarks) 以图表展示这些数字，并保留每一次测试及其脚本和原始结果。

每个数字都是三轮的中位数。基准测试的模拟服务商代替了模型服务商，因此这些数字反映的是网关本身的开销，而不是模型的开销。

## 容量

这些数字来自 64 个客户端对一个立即应答的模拟服务商发起请求，每个网关都运行在相同的两个专用核心上。同一驱动程序直连模拟服务商约可达每秒 19,600 个请求，因此每个网关都运行在它自身的极限：

| 网关 | 每秒请求数 | 延迟 p50 / p99 | 每个请求的 CPU | 内存峰值 |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.3 | 4,835 | 12.8 / 22.6 ms | 412 µs | 181 + 633 MB |
| Kaeso 0.7.3，开启请求日志 | 3,914 | 15.9 / 28.3 ms | 506 µs | 179 + 1,476 MB |
| LiteLLM Rust，`main` 分支 `d4e8f96` | 2,068 | 30.4 / 53.7 ms | 842 µs | 86 MB |
| Bifrost 2.6.0 | 2,088 | 27.9 / 68.7 ms | 954 µs | 422 MB |
| Portkey 1.15.2 | 599 | 104.1 / 159.8 ms | 1,908 µs | 290 MB |
| LiteLLM Python 1.110.2 | 39 | 1,622.0 / 1,827.0 ms | 25,484 µs | 742 MB |

每个请求的 CPU 只计网关本身。Kaeso 的内存是网关加上其数据库的内存：PostgreSQL 和 Valkey，开启请求日志时还包括 ClickHouse。其他网关运行时没有数据库。[kaeso.app/benchmarks](https://kaeso.app/benchmarks) 展示每一轮的范围。

## 附加延迟

附加延迟是请求经过网关的时间减去同一请求直连模拟服务商的时间。模拟服务商约 22 ms 应答，与快速的服务商相当。每个单元格为 p50 / p99：

| 网关 | 1 个客户端 | 4 个客户端 | 16 个客户端 |
| --- | --- | --- | --- |
| Kaeso 0.7.3 | +0.81 / +1.97 ms | +0.67 / +2.16 ms | +0.53 / +1.81 ms |
| Kaeso 0.7.3，开启请求日志 | +0.83 / +2.44 ms | +0.68 / +2.39 ms | +0.55 / +1.93 ms |
| LiteLLM Rust，`main` 分支 `d4e8f96` | +1.03 / +1.32 ms | +0.78 / +1.95 ms | +0.88 / +4.83 ms |
| Bifrost 2.6.0 | +1.28 / +1.79 ms | +1.02 / +2.60 ms | +1.01 / +4.10 ms |
| Portkey 1.15.2 | +2.32 / +6.63 ms | +1.44 / +9.36 ms | +5.9 / +30.1 ms |
| LiteLLM Python 1.110.2 | +26.5 / +29.7 ms | +87 / +110 ms | +376 / +420 ms |

没有 64 个客户端的数据。在这一负载下，模拟服务商和驱动程序共用两个核心，上限约为每秒 2,900 个请求。此时减去直连运行会低估每个网关的耗时；这一负载改由容量表衡量。

## 流式输出与智能体会话

流式测试在 16 个客户端下发送 200 个流式请求。会话测试回放一个录制的 Claude Code 会话和一个录制的 Codex 会话，各 30 轮。会话的附加时间是经过网关的总时长减去直连的总时长：

| 网关 | 首个 token 附加时间 | 分块间隔 p99 | Claude Code 会话 | Codex 会话 |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.3 | +1.21 ms | 8.7 ms | +63.4 ms | +65.6 ms |
| Kaeso 0.7.3，开启请求日志 | +1.49 ms | 9.5 ms | +71.1 ms | +68.8 ms |
| LiteLLM Rust，`main` 分支 `d4e8f96` | +1.27 ms | 40.1 ms | +64.4 ms | +54.4 ms |
| Bifrost 2.6.0 | 不可比较 | 20.2 ms | +183.9 ms | +118.8 ms |
| Portkey 1.15.2 | +28.19 ms | 9.3 ms | +114.2 ms | +120.5 ms |
| LiteLLM Python 1.110.2 | +402.27 ms | 71.2 ms | +814.8 ms | +835.7 ms |

直连模拟服务商时，分块间隔的 p99 为 8.3 ms。Bifrost 在服务商应答之前就发送第一个事件，基准测试将其计为首个 token，因此它的首 token 时间为负数。每个流式会话经过每个网关都给出了有效的工具调用。

## 其他网关领先之处

- **内存：**LiteLLM Rust 从未超过 87 MB，且无需数据库。Kaeso 最高达到 181 MB，其 PostgreSQL 和 Valkey 另占 633 MB。
- **单个客户端：**每次只发一个请求时，LiteLLM Rust 的 p99 为 +1.32 ms，Kaeso 为 +1.97 ms，而且 Kaeso 没有任何一轮更接近。
- **平手：**首个 token 时间和 Claude Code 会话都落在彼此的范围之内。
- **请求日志：**将每个请求记录到 ClickHouse 时，Kaeso 每秒处理的请求少了 19%，其数据库占用 1,476 MB。

## 自 Kaeso 0.7.2 以来

Kaeso 0.7.2 紧接在 0.7.3 之后，用同一套脚本在本次测试的机器上再次运行，分别开启和关闭请求日志。因此两个版本的比较不受机器差异影响。只有在两组各轮结果互不重叠时，差异才算数；只测量一次的值（如 CPU 或内存峰值）只有变化超过十分之一时才算数：

| 指标 | 0.7.2 | 0.7.3 | 解读 |
| --- | --- | --- | --- |
| 满载时每秒请求数 | 4,678 | 4,835 | 更高，超出两组各轮范围 |
| 同上，开启请求日志 | 3,849 | 3,914 | 相同：各轮结果重叠 |
| 满载时每个请求的 CPU | 427 µs | 412 µs | 相同：变化在十分之一以内 |
| 中位数附加延迟，1 个客户端 | +0.76 ms | +0.81 ms | 更高，超出两组各轮范围 |
| 中位数附加延迟，4 个客户端 | +0.55 ms | +0.67 ms | 更高，超出两组各轮范围 |
| 中位数附加延迟，16 个客户端 | +0.56 ms | +0.53 ms | 相同：各轮结果重叠 |
| p99 附加延迟，16 个客户端 | +1.54 ms | +1.81 ms | 相同：各轮结果重叠 |
| 分块间隔 p99 | 8.7 ms | 8.7 ms | 相同：各轮结果重叠 |
| 满载时内存峰值，开启请求日志 | 168 MB | 179 MB | 相同：变化在十分之一以内 |
| 满载时 Kaeso 数据库的内存峰值 | 567 MB | 633 MB | 更高，超过十分之一 |

两个版本先后运行，没有交替进行。0.7.3 有一项改动位于本基准测试的路径上：在回复发出之前，网关现在会先记下该请求，使[来源](../concepts/sources.md)上报的副本只计一次；对于没有来源的组织，这只是在独立任务中进行的一次缓存检查。容量 3% 的差异和中位数 0.05 至 0.12 ms 的差异是源于这项改动，还是源于两次运行之间这一小时里机器的变化，各轮结果无法说明。更早的 [0.7.2](https://kaeso.app/zh-CN/benchmarks/0.7.2) 和 [0.7.0](https://kaeso.app/zh-CN/benchmarks/0.7.0) 测试运行在其他同型号机器上，连同脚本和原始结果仍然公开。

## 每个网关做了什么

每个网关都按其项目的发布方式运行，对接使用 Anthropic Messages 格式的模拟服务商：

| 网关 | 每个请求检查的内容 | 保存的内容 |
| --- | --- | --- |
| Kaeso 0.7.3 | 项目的 [API 密钥](../console/api-keys.md)，以及项目允许的模型 | 用量存入 PostgreSQL；无请求日志 |
| Kaeso 0.7.3，开启请求日志 | 同上 | 用量存入 PostgreSQL，每个请求存入 ClickHouse |
| LiteLLM Rust，`main` 分支 `d4e8f96`，未发布 | 配置中的一个主密钥 | 不保存；无数据库 |
| LiteLLM Python 1.110.2 | 配置中的一个主密钥 | 不保存；无数据库 |
| Bifrost 2.6.0 | 不检查密钥 | 不保存；日志与存储均关闭 |
| Portkey 1.15.2 | 不检查密钥 | 不保存；日志关闭 |

## 测试方法

- **机器：**DigitalOcean CPU-Optimized c-4，4 个专用 vCPU（Intel Xeon Platinum 8168，2.7 GHz），8 GB 内存，Ubuntu 24.04 与 Docker，测试日期为 2026 年 11 月 25 日。之前几次测试所在的区域已不再提供 c-4，因此这台机器位于另一个区域；同一驱动程序在这里直连模拟服务商的每秒请求数少了四分之一以上，每个网关的数字都更低。
- **核心：**每个网关运行在 Docker 中，使用主机网络，固定在两个核心上。模拟服务商、负载驱动程序和 Kaeso 的数据库运行在另外两个核心上。
- **基准测试：**`BerriAI/ai-gateway-bench`，提交 `44365ca`，模拟服务商与驱动程序均未改动：`load-probe` 用于请求，`run_http_bench.py` 用于流式输出，`session_runner.py` 用于会话。
- **模拟服务商：**20 ms 后发出首个 token，随后以 5 ms 的间隔发出 40 个 token。容量测试中立即应答。
- **负载：**1、4、16 和 64 个客户端，每轮分别发送 1,000、2,000、4,000 和 16,000 个请求。
- **轮次：**三轮，每轮网关顺序重新随机。每个网关都在一次形态相同的直连运行之后立即测量。
- **CPU 与内存：**网关容器中的每个进程，每秒读取四次；其数据库同样读取。

各镜像按以下摘要运行：

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.3@sha256:e070906766362fc298babb51d2d0ee9a60deb07cbf30aac2e408f6d193f91553
ghcr.io/logicplanes/kaeso-gateway:0.7.2@sha256:7db4ee52773bc546746fb5e3d513457929d1782fdb89795f7b9c9f406a94ae79
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
curl -fsSLO https://kaeso.app/benchmarks/runs/0.7.3/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
# 再次运行 Kaeso 0.7.2，用于上文的比较
ai-gateway-bench/.venv/bin/python run072.py kaeso kaeso-logs
ai-gateway-bench/.venv/bin/python cap072.py kaeso kaeso-logs
```

所有网关跑完三轮约需 90 分钟，容量测试另需 30 分钟。结果保存在 `/root/out` 和 `/root/cap` 中，0.7.2 的结果保存在 `/root/out072` 和 `/root/cap072` 中。绝对数值因机器而异，请比较同一次运行中测得的网关。

## 下一步

- [系统要求](requirements.md)：按预期流量规划服务器。
- [多个网关](scaling.md)：一个网关不够时运行更多网关。
- [网关基准测试](../development/benchmark.md)：单独测量 Kaeso 在流式回复、断开连接和慢速应用下的表现。
