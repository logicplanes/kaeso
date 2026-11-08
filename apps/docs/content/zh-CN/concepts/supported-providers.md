---
title: 支持的服务商
description: 查阅 Kaeso 调用的每个服务商，包括其 ID、调用格式、主机以及密钥的获取位置。
order: 2
---

Kaeso 内置了以下服务商，按名称排列。如何使用服务商以及它们之间的差异，见[服务商](providers.md)。组织还可以添加使用其中某种格式的自有服务器（[自定义服务商](../console/custom-providers.md)）。

## 各列说明

- **服务商。** 控制台显示的名称，后跟 ID。ID 用于在模型名称（如 `groq/llama-3.3-70b-versatile`）和[设置](../self-hosting/configuration.md#服务商设置)中指定服务商。
- **格式。** Kaeso 调用该服务商所用的 API 格式。模型使用第一种格式，除非其**发送格式**指定了另一种（[模型的参数](../console/models-and-aliases.md#更改模型的参数)）。**Embeddings** 和 **Gemini Embeddings** 表示该服务商提供嵌入模型，可用于 `/v1/embeddings`。**Token 计数**表示它可以为 `/v1/messages/count_tokens` 和 `/v1/responses/input_tokens` 计算 Token 数。
- **主机。** 服务商默认基础 URL 的主机，实例必须能够访问它。如果您[设置了基础 URL](../self-hosting/configuration.md#服务商设置)，则以该地址为准。
- **密钥。** 在哪里创建密钥，以及密钥的开头（[服务商密钥](../console/provider-keys.md)）。

## 服务商列表

| 服务商 | 格式 | 主机 | 密钥 |
| --- | --- | --- | --- |
| Anthropic `anthropic` | Messages、Token 计数 | `api.anthropic.com` | [platform.claude.com](https://platform.claude.com/settings/keys)，`sk-ant-` |
| Cerebras `cerebras` | Chat Completions | `api.cerebras.ai` | [cloud.cerebras.ai](https://cloud.cerebras.ai)，`csk-` |
| Cohere `cohere` | Chat Completions、Embeddings | `api.cohere.ai` | [dashboard.cohere.com](https://dashboard.cohere.com/api-keys)，无固定前缀 |
| DeepSeek `deepseek` | Chat Completions | `api.deepseek.com` | [platform.deepseek.com](https://platform.deepseek.com/api_keys)，`sk-` |
| Google AI Studio `google` | Gemini API、Gemini Embeddings、Token 计数 | `generativelanguage.googleapis.com` | [aistudio.google.com](https://aistudio.google.com/apikey)，无固定前缀 |
| Groq `groq` | Chat Completions | `api.groq.com` | [console.groq.com](https://console.groq.com/keys)，`gsk_` |
| MiniMax `minimax` | Messages | `api.minimax.io` | [platform.minimax.io](https://platform.minimax.io/user-center/basic-information/interface-key)，`sk-api-` |
| Mistral `mistral` | Chat Completions、Embeddings | `api.mistral.ai` | [console.mistral.ai](https://console.mistral.ai/api-keys)，无固定前缀 |
| Moonshot `moonshotai` | Chat Completions | `api.moonshot.ai` | [platform.kimi.ai](https://platform.kimi.ai/console/api-keys)，`sk-` |
| Ollama Cloud `ollama-cloud` | Chat Completions | `ollama.com` | [ollama.com](https://ollama.com/settings/keys)，无固定前缀 |
| OpenAI `openai` | Responses、Chat Completions、Embeddings、Token 计数 | `api.openai.com` | [platform.openai.com](https://platform.openai.com/api-keys)，`sk-` |
| OpenRouter `openrouter` | Chat Completions、Embeddings | `openrouter.ai` | [openrouter.ai](https://openrouter.ai/settings/keys)，`sk-or-v1-` |
| QwenCloud `alibaba` | Chat Completions、Embeddings | `ap-southeast-1.maas.qwencloudapi.com` | [home.qwencloud.com](https://home.qwencloud.com/api-keys)，`sk-` |
| SpaceX `xai` | Responses、Chat Completions | `api.x.ai` | [console.x.ai](https://console.x.ai/team/default/api-keys)，`xai-` |
| Together AI `togetherai` | Chat Completions、Embeddings | `api.together.ai` | [api.together.ai](https://api.together.ai/settings/projects/~current/api-keys)，`tgp_v1_` |
| Vercel `vercel` | Chat Completions、Responses、Messages、Embeddings | `ai-gateway.vercel.sh` | [vercel.com](https://vercel.com/d?to=%2F%5Bteam%5D%2F%7E%2Fai-gateway%2Fapi-keys)，`vck_` |
| Z.ai `zai` | Messages | `api.z.ai` | [z.ai](https://z.ai/manage-apikey/apikey-list)，无固定前缀 |

## 补充说明

关于部分服务商的其他须知事项。服务商会改名或拒绝的每个 Chat Completions 字段，见[服务商配置](../development/kpp/openai-chat-providers.md#服务商配置)：

- **Cerebras。** 即 Cerebras Inference，提供 `gpt-oss-120b`、`qwen-3.8-27b` 等模型。`developer` 消息会作为 `system` 消息发送给它，终端用户的 ID 以 `user` 发送，Claude Code 或 Codex 在第一轮之后发送的系统消息的文本并入下一个用户轮次，没有参数的函数带上空的参数 schema。Grok CLI 发回的 `reasoning_content` 以 `reasoning` 到达它。它拒绝以 assistant 消息结尾的对话，因为 Cerebras 会重新回答，也拒绝 assistant 消息上的 `refusal`。Cerebras 对任何它不认识的字段（例如 `store`）返回 400，并且只接受 data URI 形式的图像。模型的思考内容可到达任何格式的应用；`gpt-oss-120b` 在 `reasoning_effort` 为 `low`、`medium` 或 `high` 时思考，`qwen-3.8-27b` 还接受 `none`，其他强度由 Kaeso 拒绝。
- **Cohere。** 即 Cohere 的 [Compatibility API](https://docs.cohere.com/docs/compatibility-api)，它只覆盖 Cohere 自身 API 的一部分：无法通过它使用文档、引用和重排序。Cohere 拒绝部分字段，例如 `logit_bias` 和 `parallel_tool_calls: false`；单个 `stop` 字符串会以列表形式发送给它。它的推理模型只支持 `reasoning_effort` 为 `none` 或 `high`。它的 Embed 模型（例如 `embed-v4.0`）不在 KPMR 中：请将其添加为以 Embeddings 发送的自定义模型，并填写价格。
- **DeepSeek。** `developer` 消息会作为 `system` 消息发送给它。
- **Google AI Studio。** 即 Google AI Studio 的 Gemini API，提供 `gemini-3.5-flash`、`gemini-3.1-pro-preview` 等 Gemini 模型、Gemma 模型，以及嵌入模型 `gemini-embedding-2` 和 `gemini-embedding-001`。Kaeso 以 Gemini 自己的格式调用它，密钥放在 `x-goog-api-key` 中，因此各种格式的应用都能访问它，Gemini 的思维签名在工具调用之间也得以保留。Gemini 3 模型按 KPMR 为其列出的推理强度级别思考，以 `thinkingLevel` 发送。Gemini 2.5 模型没有列出级别：Anthropic 应用的 `budget_tokens` 以 `thinkingBudget` 发送，其他应用的推理强度则被省略，并在 `kaeso-left-out` 中注明。托管网页搜索会被省略，因为 Gemini 的 `googleSearch` 不接受它的任何设置。AI Studio 的密钥以 `AIza` 或 `AQ.` 开头。免费层级的密钥每个模型每天只有很少的配额；超出后 Google 返回 429，应用会收到速率限制错误。**检查可用性**能识别错误的密钥。Google Cloud 的 Gemini API Vertex AI 不属于此服务商。
- **Groq。** `developer` 消息会作为 `system` 消息发送给它；它拒绝部分字段，例如 `logit_bias`。
- **MiniMax。** 即 MiniMax 的 Anthropic 兼容 API，提供 `MiniMax-M3` 和 `MiniMax-M3.1-Flash`，这是 MiniMax 在该接口上提供的仅有的模型；它的 M2 模型已下线。Kaeso 以 Messages 格式调用它，密钥放在 `x-api-key` 中，因此各种格式的应用都能访问它。Kaeso 拒绝 MiniMax 接受却不作处理的内容：`top_k`、`stop_sequences`（Chat 应用的 `stop`）、`mcp_servers`、`container`、不为 1 的 `temperature`、不为 0.95 的 `top_p`，以及不为 `auto` 的 `tool_choice`。Claude Code 只清除思考内容的 `context_management` 会被省略，并在 `kaeso-left-out` 中注明。图片可以通过 URL 或 base64 发送，文档由 MiniMax 自己拒绝。`MiniMax-M3.1-Flash` 始终思考，并拒绝关闭思考。MiniMax 自己为思考块签名，因此 Anthropic 应用应只把含有这些思考块的对话发给 MiniMax 模型；Anthropic 会拒绝它们的签名。Kaeso 拒绝 Token 计数，因为 MiniMax 返回的格式与 Anthropic 不同。在中国大陆，请将 MiniMax 的[基础 URL](../self-hosting/configuration.md#服务商设置)设为 `https://api.minimax.cn/anthropic`，并使用 platform.minimax.cn 的密钥。
- **Mistral。** `developer` 消息会作为 `system` 消息发送给它，部分字段以它自己的名称发送，例如 `seed` 改为 `random_seed`；它拒绝其他一些字段（例如 `logit_bias`），也拒绝以助手消息结尾的对话。
- **Moonshot。** 即 Kimi API，提供 Moonshot 的 Kimi 模型，例如 `kimi-k3` 和 `kimi-k2.6`。`developer` 消息会作为 `system` 消息发送给它；应用预填的助手消息会带上 Moonshot 的 `partial: true` 发送，使回复接着该消息续写。它拒绝部分字段，例如 `parallel_tool_calls: false` 和 `store`，因为 Moonshot 接受这些字段却不作处理。Kimi 模型的采样参数是固定的：Kaeso 在发送前拒绝 1 以外的 `temperature`，`top_p`、`n` 和惩罚参数的其他值则由 Moonshot 拒绝。模型的推理内容会以各种格式送达应用。K3 模型的 `reasoning_effort` 为 `low`、`high` 或 `max`，Kaeso 拒绝其他级别；`kimi-k2.6` 在 `none` 时不思考，`kimi-k2.7-code` 始终思考。Kimi 模型在思考时拒绝 `tool_choice: "required"`。
- **Ollama Cloud。** 由 Ollama 在自己的服务器上运行的模型，例如 `gpt-oss:120b`。模型 ID 保留 Ollama 在冒号后的标签。每个应用的输出上限都以 `max_tokens` 发送给它，因为 Ollama 会忽略 `max_completion_tokens`；它拒绝 `logprobs`、`top_logprobs` 和 `logit_bias`，因为 Ollama 接受这些字段却不作处理。模型的推理内容会以各种格式送达应用。Ollama 的免费套餐只包含部分模型：对其他模型，Ollama 返回 402，应用会将其作为被拒绝的服务商密钥收到。Ollama 列出模型时不检查密钥，因此**检查可用性**无法发现错误的密钥；使用该密钥的第一个请求会报告密钥被拒绝。本地 Ollama 服务器不需要密钥；请将其添加为使用 Chat Completions 的[自定义服务商](../console/custom-providers.md)。
- **OpenAI。** 服务商密钥也可以是一个 ChatGPT 账户，其请求按 OpenAI 的相应规则使用该账户的 ChatGPT 套餐（[使用 ChatGPT 账户登录](../console/provider-keys.md#使用-chatgpt-账户登录)）。
- **OpenRouter。** 模型 ID 包含模型开发者，例如 `anthropic/claude-sonnet-4.5`。请以 `openrouter/anthropic/claude-sonnet-4.5` 的形式调用，这样请求不会发往该开发者自己的服务商（[请求如何指定模型](../console/models-and-aliases.md#请求如何指定模型)）。Kaeso 按目录中的价格记录费用，而不是按 OpenRouter 在 `usage.cost` 中报告的费用。OpenRouter 列出模型时不检查密钥，因此**检查可用性**无法发现错误的密钥；使用该密钥的第一个请求会报告密钥被拒绝。
- **QwenCloud。** 阿里巴巴在中国内地以外的模型平台，前身为阿里云百炼国际站（Model Studio），此处为其新加坡地域，提供 `qwen3.7-flash` 等通义千问模型，以及 `kimi-k3` 等其他厂商的模型。密钥只能在创建它的地域使用。对于中国（北京）地域（KPMR 中为 `alibaba-cn`），请将 QwenCloud 的[基础 URL](../self-hosting/configuration.md#服务商设置) 设为 `https://dashscope.aliyuncs.com/compatible-mode`，并使用该地域的密钥；一个实例只能连接一个地域。`developer` 消息会以 `system` 消息发送给它；QwenCloud 接受但不处理的字段（例如 `logit_bias` 和 `web_search_options`）会被拒绝。应用的思考请求以 `enable_thinking` 发送给模型，Anthropic 应用的 `budget_tokens` 以 `thinking_budget` 发送，推理强度只发送给列出了强度级别的模型，例如 `qwen3.8-max`。Chat 应用的 `max_tokens` 只限制回答，不限制思考；`max_completion_tokens` 同时限制两者。有些模型只能思考，会拒绝关闭思考，`qwq-plus` 只回答流式请求：应用会收到 QwenCloud 的 400。它的嵌入模型（例如 `text-embedding-v4`）不在 KPMR 中：请将其添加为以 Embeddings 发送的自定义模型，并填写价格。
- **SpaceX。** 即 xAI 的 API，提供 Grok 等模型。作为实验性选项，服务商密钥也可以是套餐包含 API 访问权限的 Grok 账户（[使用 Grok 账户登录](../console/provider-keys.md#使用-grok-账户登录)）。
- **Vercel。** 即 Vercel AI Gateway，另一个网关，其自身的路由和回退在 Kaeso 之后运行。模型 ID 包含模型开发者，例如 `openai/gpt-5-nano`。请以 `vercel/openai/gpt-5-nano` 的形式调用，这样请求不会发往该开发者自己的服务商（[请求如何指定模型](../console/models-and-aliases.md#请求如何指定模型)）。模型的**发送格式**可以选择 Vercel 同样提供的 Responses 或 Messages；通过 Messages，Vercel 会把 Anthropic 的 `cache_control` 传给 Claude 模型。Kaeso 按目录中的价格记录费用，而不是按 Vercel 在 `usage.cost` 中报告的费用，后者不含 Vercel 自己的附加费。Vercel 只为 Claude 模型精确计数 Token，对其余模型只做估算，因此 Kaeso 拒绝为任何 Vercel 模型计数 Token。Vercel 回答模型信息时不检查密钥，因此**检查可用性**无法发现错误的密钥；使用该密钥的第一个请求会报告密钥被拒绝。免费套餐的 Vercel 账户无法调用 Claude 等付费模型：Vercel 返回 403，应用会将其视为服务商密钥被拒绝，请求日志中保留 Vercel 给出的原因。
- **Z.ai。** 即 Z.ai 的 Anthropic 兼容 API，提供 `glm-5.3`、`glm-4.6` 等 GLM 模型。Kaeso 以 Messages 格式调用它，密钥放在 `x-api-key` 中，因此各种格式的应用都能访问它，Anthropic 应用的请求按应用原样发送。Z.ai 没有说明它接受哪些 Anthropic 字段，因此 Kaeso 会全部发送，是否拒绝由 Z.ai 决定；重复的提示词由 Z.ai 自动缓存。Z.ai 发送自己的错误码：余额不足（`1113`）以被拒绝的服务商密钥送达应用，而不是速率限制；服务过载（`1305`）以过载送达；提示词过长（`1261`）也以提示词过长送达，这样 Claude Code 会压缩对话。Kaeso 拒绝 Token 计数，因为 Z.ai 没有提供相关说明。**检查可用性**能发现错误的密钥；Z.ai 没有说明模型列表，因此能否发现 Z.ai 不提供的模型尚未验证。Z.ai 按密钥类型对每次调用计费，并且只允许在其列出的编程工具中使用 GLM Coding Plan 密钥，因此请使用标准 API 密钥。在中国大陆，请将 Z.ai 的[基础 URL](../self-hosting/configuration.md#服务商设置)设为 `https://open.bigmodel.cn/api/anthropic`，并使用 bigmodel.cn 的密钥。

## 商标

所有商标和标志均归其各自所有者所有。Kaeso 与这些服务商均无关联，也未获得其认可或赞助。控制台显示服务商的标志，仅用于标明密钥、模型或请求所属的服务商。Anthropic、Cerebras、Cohere、Google、Groq、Z.ai、MiniMax 以及 QwenCloud 的所有者 Alibaba Cloud 在其条款中限制使用其标志；Kaeso 仅出于此目的显示它们的标志，保留所有者的形状和颜色，只在颜色需要在 Kaeso 的浅色和深色背景上保持可见时才做必要的调整。Mistral 的品牌页面要求不要给其图标重新着色或加框；Kaeso 以 Mistral 自己的颜色显示该图标，放在与其他标志相同的中性底块上。[NOTICE.md](https://github.com/logicplanes/kaeso/blob/dev/NOTICE.md#trademarks) 列出了每个标志的来源。
