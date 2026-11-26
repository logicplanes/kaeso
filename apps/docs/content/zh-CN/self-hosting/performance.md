---
title: 性能
description: 了解在 BerriAI 的基准测试中，Kaeso 与 LiteLLM、Bifrost 和 Portkey 相比增加了多少时间、CPU 和内存。
order: 14
---

Kaeso 0.7.4 在 [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench) 上运行，这是 LiteLLM 背后的公司 BerriAI 发布的基准测试。LiteLLM、Bifrost 和 Portkey 在同一台机器上一同运行，Kaeso 此前的每个 0.7 版本也是如此。您可以用这些数字来规划服务器，并将 Kaeso 与您现在运行的网关进行比较。[kaeso.app/benchmarks](https://kaeso.app/zh-CN/benchmarks) 以图表展示这些数字，并保留每一次测试及其脚本和原始结果。

每个数字都是三轮的中位数。基准测试的模拟服务商代替了模型服务商，因此这些数字反映的是网关本身的开销，而不是模型的开销。

## 容量

这些数字来自 64 个客户端对一个立即应答的模拟服务商发起请求，每个网关都运行在相同的两个专用核心上。同一驱动程序直连模拟服务商约可达每秒 23,200 个请求，因此每个网关都运行在它自身的极限：

| 网关 | 每秒请求数 | 延迟 p50 / p99 | 每个请求的 CPU | 内存峰值 |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | 4,551 | 13.8 / 21.6 ms | 437 µs | 163 + 594 MB |
| Kaeso 0.7.4，开启请求日志 | 3,962 | 15.8 / 27.2 ms | 500 µs | 188 + 1,489 MB |
| LiteLLM Rust，`main` 分支 `d4e8f96` | 2,955 | 21.4 / 35.5 ms | 642 µs | 87 MB |
| Bifrost 2.6.0 | 2,242 | 24.7 / 63.9 ms | 887 µs | 414 MB |
| Portkey 1.15.2 | 619 | 101.2 / 148.6 ms | 1,829 µs | 303 MB |
| LiteLLM Python 1.110.2 | 55 | 1,162.0 / 1,331.9 ms | 18,286 µs | 742 MB |

每个请求的 CPU 只计网关本身。Kaeso 的内存是网关加上其数据库的内存：PostgreSQL 和 Valkey，开启请求日志时还包括 ClickHouse。其他网关运行时没有数据库。[kaeso.app/benchmarks](https://kaeso.app/benchmarks) 展示每一轮的范围。

## 附加延迟

附加延迟是请求经过网关的时间减去同一请求直连模拟服务商的时间。模拟服务商约 22 ms 应答，与快速的服务商相当。每个单元格为 p50 / p99：

| 网关 | 1 个客户端 | 4 个客户端 | 16 个客户端 |
| --- | --- | --- | --- |
| Kaeso 0.7.4 | +0.94 / +1.96 ms | +0.92 / +1.81 ms | +0.64 / +1.86 ms |
| Kaeso 0.7.4，开启请求日志 | +0.91 / +2.24 ms | +0.99 / +2.53 ms | +0.77 / +2.08 ms |
| LiteLLM Rust，`main` 分支 `d4e8f96` | +1.05 / +1.16 ms | +0.97 / +1.67 ms | +0.82 / +2.43 ms |
| Bifrost 2.6.0 | +1.39 / +1.75 ms | +1.31 / +2.76 ms | +0.89 / +5.03 ms |
| Portkey 1.15.2 | +2.63 / +6.76 ms | +2.35 / +9.35 ms | +5.1 / +24.4 ms |
| LiteLLM Python 1.110.2 | +20.4 / +22.2 ms | +65.2 / +76.7 ms | +260 / +284 ms |

没有 64 个客户端的数据。在这一负载下，模拟服务商和驱动程序共用两个核心，上限约为每秒 2,900 个请求。此时减去直连运行会低估每个网关的耗时；这一负载改由容量表衡量。

## 流式输出与智能体会话

流式测试在 16 个客户端下发送 200 个流式请求。会话测试回放一个录制的 Claude Code 会话和一个录制的 Codex 会话，各 30 轮。会话的附加时间是经过网关的总时长减去直连的总时长：

| 网关 | 首个 token 附加时间 | 分块间隔 p99 | Claude Code 会话 | Codex 会话 |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | +1.08 ms | 8.0 ms | +50.7 ms | +52.1 ms |
| Kaeso 0.7.4，开启请求日志 | +1.11 ms | 9.0 ms | +60.2 ms | +80.7 ms |
| LiteLLM Rust，`main` 分支 `d4e8f96` | +1.32 ms | 40.3 ms | +54.3 ms | +59.3 ms |
| Bifrost 2.6.0 | 不可比较 | 20.4 ms | +168.8 ms | +104.8 ms |
| Portkey 1.15.2 | +28.20 ms | 9.8 ms | +121.0 ms | +125.2 ms |
| LiteLLM Python 1.110.2 | +227.63 ms | 50.5 ms | +642.6 ms | +647.0 ms |

直连模拟服务商时，分块间隔的 p99 为 7.7 ms。Bifrost 在服务商应答之前就发送第一个事件，基准测试将其计为首个 token，因此它的首 token 时间为负数。每个流式会话经过每个网关都给出了有效的工具调用。

## 其他网关领先之处

- **内存：**LiteLLM Rust 从未超过 87 MB，且无需数据库。Kaeso 最高达到 163 MB，其 PostgreSQL 和 Valkey 另占 594 MB。
- **单个客户端：**每次只发一个请求时，LiteLLM Rust 的 p99 为 +1.16 ms，Kaeso 为 +1.96 ms，而且 Kaeso 没有任何一轮更接近。
- **请求日志：**将每个请求记录到 ClickHouse 时，Kaeso 每秒处理的请求少了 13%，其数据库占用 1,489 MB。

## 所有 0.7 版本，同一台机器

在本次测试中，Kaeso 0.7.0、0.7.1、0.7.2、0.7.3 和 0.7.4 各自作为独立网关，与其他网关在相同的随机顺序轮次中运行，因此各版本可以逐项对比，不受机器或时间先后的影响。只有两个版本的各轮结果互不重叠时，一步变化才算数；CPU 或内存峰值这类只测一次的值，变化超过十分之一才算数：

| 版本 | 每秒请求数 | 开启请求日志 | 附加延迟 p50，1 / 4 / 16 个客户端 | 附加延迟 p99，16 个客户端 | 每个请求的 CPU | 内存峰值 |
| --- | --- | --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | 4,551 | 3,962 | +0.94 / +0.92 / +0.64 ms | +1.86 ms | 437 µs | 163 + 594 MB |
| Kaeso 0.7.3 | 5,204 | 4,371 | +1.02 / +1.06 / +0.73 ms | +1.67 ms | 384 µs | 102 + 543 MB |
| Kaeso 0.7.2 | 4,666 | 4,032 | +0.92 / +0.90 / +0.66 ms | +1.70 ms | 428 µs | 178 + 579 MB |
| Kaeso 0.7.1 | 4,413 | 3,781 | +0.96 / +0.99 / +0.83 ms | +1.90 ms | 453 µs | 110 + 565 MB |
| Kaeso 0.7.0 | 4,748 | 3,891 | +0.98 / +0.99 / +0.84 ms | +2.03 ms | 421 µs | 113 + 283 MB |

- **0.7.3 到 0.7.4：**0.7.4 撤回了 0.7.3 给每次回复增加的等待，它在 1 个和 4 个客户端时附加的中位数再次低于 0.7.3。0.7.3 在每一轮中处理的请求都是所有版本中最多的；0.7.4 少了 13%，每个请求的 CPU 多了 14%，落在 0.7.2 的各轮范围之内。
- **0.7.2 到 0.7.3：**每秒请求更多、每个请求的 CPU 更少，但 1、4 和 16 个客户端时的中位数更高。
- **0.7.1 到 0.7.2：**无论是否开启请求日志，每秒请求都更多；1 个和 4 个客户端时的中位数更低。
- **0.7.0 到 0.7.1：**每秒请求少了 7%，数据库的内存峰值翻了一倍，此后每个版本都保持在这一水平。

各轮结果显示了每一项变化，但无法说明其原因。更早的 [0.7.3](https://kaeso.app/zh-CN/benchmarks/0.7.3)、[0.7.2](https://kaeso.app/zh-CN/benchmarks/0.7.2) 和 [0.7.0](https://kaeso.app/zh-CN/benchmarks/0.7.0) 测试运行在其他同型号机器上，连同脚本和原始结果仍然公开。
## 每个网关做了什么

每个网关都按其项目的发布方式运行，对接使用 Anthropic Messages 格式的模拟服务商：

| 网关 | 每个请求检查的内容 | 保存的内容 |
| --- | --- | --- |
| Kaeso 0.7.4 | 项目的 [API 密钥](../console/api-keys.md)，以及项目允许的模型 | 用量存入 PostgreSQL；无请求日志 |
| Kaeso 0.7.4，开启请求日志 | 同上 | 用量存入 PostgreSQL，每个请求存入 ClickHouse |
| LiteLLM Rust，`main` 分支 `d4e8f96`，未发布 | 配置中的一个主密钥 | 不保存；无数据库 |
| LiteLLM Python 1.110.2 | 配置中的一个主密钥 | 不保存；无数据库 |
| Bifrost 2.6.0 | 不检查密钥 | 不保存；日志与存储均关闭 |
| Portkey 1.15.2 | 不检查密钥 | 不保存；日志关闭 |

## 测试方法

- **机器：**DigitalOcean CPU-Optimized c-4，4 个专用 vCPU（Intel Xeon Platinum 8280，2.7 GHz），8 GB 内存，Ubuntu 24.04 与 Docker，测试日期为 2026 年 11 月 25 日至 26 日。测试前关闭了它的计划任务、软件包更新、snapd 和服务商的代理程序，也关闭了透明大页和交换。同一驱动程序在这里直连模拟服务商约可达每秒 23,200 个请求，而 0.7.0 和 0.7.2 测试的机器约可达 28,000 个，因此每个网关的数字都低于那两次测试。
- **核心：**每个网关运行在 Docker 中，使用主机网络，固定在两个核心上。模拟服务商、负载驱动程序和 Kaeso 的数据库运行在另外两个核心上。
- **基准测试：**`BerriAI/ai-gateway-bench`，提交 `44365ca`，模拟服务商与驱动程序均未改动：`load-probe` 用于请求，`run_http_bench.py` 用于流式输出，`session_runner.py` 用于会话。
- **模拟服务商：**20 ms 后发出首个 token，随后以 5 ms 的间隔发出 40 个 token。容量测试中立即应答。
- **负载：**1、4、16 和 64 个客户端，每轮分别发送 1,000、2,000、4,000 和 16,000 个请求。
- **轮次：**三轮，每轮网关顺序重新随机。每个网关都在一次形态相同的直连运行之后立即测量。
- **CPU 与内存：**网关容器中的每个进程，每秒读取四次；其数据库同样读取。

各镜像按以下摘要运行：

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

LiteLLM Rust 尚无已发布的镜像。它由 `BerriAI/litellm` 的提交 `d4e8f96` 使用 `rust:1.98.0` 构建。

> [!IMPORTANT]
> 这次基准测试由 Kaeso 的作者在一台机器上完成。BerriAI 称其自己公布的结果为单机且由厂商测试，这里同样如此。在依赖这些结果之前，请自行复测。

## 自己运行

使用一台全新的四核 Ubuntu 24.04 机器，并以 root 身份运行。安装脚本会安装 Docker，按相同提交构建基准测试和 LiteLLM Rust，并按摘要拉取每个镜像：

```bash
curl -fsSLO https://kaeso.app/benchmarks/runs/0.7.4/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
```

所有网关跑完三轮约需两小时，容量测试另需 20 分钟。结果保存在 `/root/out` 和 `/root/cap` 中。每个 Kaeso 版本都是一个独立网关，即 `kaeso-0.7.4`、`kaeso-logs-0.7.4` 等；在 `run.py` 后写上网关名称即可只测试这些网关。绝对数值因机器而异，请比较同一次运行中测得的网关。

## 下一步

- [系统要求](requirements.md)：按预期流量规划服务器。
- [多个网关](scaling.md)：一个网关不够时运行更多网关。
- [网关基准测试](../development/benchmark.md)：单独测量 Kaeso 在流式回复、断开连接和慢速应用下的表现。
