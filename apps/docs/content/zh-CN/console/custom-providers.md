---
title: 自定义服务商
description: 将您自己的服务器，例如自托管的 vLLM 或内部代理，添加为组织的服务商。
order: 4
---

自定义服务商是组织自行添加的服务器，例如自托管的 vLLM、Ollama、LM Studio、TGI 或 SGLang，或者内部代理。Kaeso 以它已支持翻译的某种 API 格式调用该服务器。应用调用其模型的方式与调用任何[支持的服务商](../concepts/supported-providers.md)相同，可使用任意格式，并遵循相同的规则。

## 添加自定义服务商

1. 在**服务商**页面上打开**自定义服务商**，选择**添加自定义服务商**。
2. 选择服务器运行的软件，例如 **vLLM**、**Ollama** 或 **LM Studio**（[服务器软件](#服务器软件)），或选择**其他服务器或代理**，然后选择**继续**。
3. 填写服务器的**基础 URL**，例如 `https://vllm.example.com/v1`，以及它的 **API 密钥**。服务器不使用密钥时，请将密钥留空。
4. 选择**检测设置**。Kaeso 会向服务器询问如何调用它（[Kaeso 检测的内容](#kaeso-检测的内容)）。
5. 检查 Kaeso 检测到的内容：**名称**、**ID**、**基础 URL**、**服务器**、**API 格式**、它是否**同时提供嵌入**或**支持 Token 计数**，以及它接收**密钥**的方式。修改不正确的内容。
6. 选择**添加自定义服务商**。Kaeso 会先向服务器核实您修改过的设置，然后添加该服务商，并将密钥保存为它的服务商密钥。

如果没有**添加服务商密钥**权限，Kaeso 只用该密钥检测设置；拥有该权限的人可以在**服务商密钥**中添加它（[服务商密钥](provider-keys.md)）。最后将该服务商的模型添加到项目中（[添加其模型](#添加其模型)）。

**ID** 以服务器的主机名为起点；对于 `localhost`、`10.0.0.5` 或 `host.docker.internal` 这类地址，则以其软件名称为起点，并在您修改之前跟随名称生成。模型名称以它开头，例如 `research-vllm/llama-3.3-70b`，且它不可更改。ID 由小写字母和数字组成，中间用单个连字符分隔，最多 40 个字符。它不能是内置服务商的 ID，也不能是 [KPMR](../concepts/kpmr.md) 中列出的服务商的 ID，因为 Kaeso 将来可能内置该服务商。每个组织的 ID 相互独立，因此两个组织都可以有 `research-vllm`。

与所有服务商一样，Kaeso 会在基础 URL 后加上格式的路径，例如 `/v1/chat/completions`。因此它会去掉末尾的 `/v1` 或粘贴进来的端点，例如 `/v1/chat/completions`，并为没有协议的地址补上 `https://`。`/v1` 之前的路径会保留：`https://proxy.example.com/llm/v1` 接收的请求地址为 `https://proxy.example.com/llm/v1/chat/completions`。API 不在 `/v1` 之下的服务器暂时无法添加。

## Kaeso 检测的内容

Kaeso 使用不产生费用的请求来查找设置。它查询服务器的模型列表 `GET /v1/models`，分别以 `Authorization: Bearer` 和 `x-api-key` 发送密钥。然后它向每种格式的路径发送空的请求体，服务器会在生成或计费之前拒绝这类请求。

| 设置 | Kaeso 如何检测 |
| --- | --- |
| 密钥 | 模型列表接受的方式；Anthropic 形式的列表优先使用 `x-api-key`。没有密钥时为**不使用密钥**。 |
| API 格式 | 列表的形式以及有响应的路径：`/v1/chat/completions`、`/v1/responses` 或 `/v1/messages`。提供 Chat Completions 的服务器会使用它，即使它也提供 Responses。 |
| 同时提供嵌入 | `/v1/embeddings` 是否有响应。 |
| 支持 Token 计数 | 对于 Responses 和 Messages，`/v1/responses/input_tokens` 或 `/v1/messages/count_tokens` 是否有响应。 |
| 服务器 | 仅限**其他服务器或代理**：模型列表给出的所有者，例如 `vllm` 或 `sglang`；`server` 响应头，例如 llama.cpp 的；或对软件自身端点发送的免费 GET 请求：Ollama 为 `/api/version`，Text Generation Inference 为 `/info`，LocalAI 为 `/version`。没有匹配时为**其他服务器或代理**。 |
| 模型 | 列表给出的 ID，以及列表提供的上下文窗口和输出上限。 |

没有模型列表的服务器，Kaeso 会通过有响应的路径来识别。如果 Kaeso 无法连接服务器、服务器拒绝了密钥，或它的响应不属于 Kaeso 支持的任何格式，对话框会在需要修改的字段旁说明原因，且不会添加任何内容。

## 选择格式

Kaeso 会选择它检测到的格式。如需修改，请选择您的服务器所使用的格式：

| 您的服务器 | API 格式 |
| --- | --- |
| vLLM、Ollama、LM Studio、TGI、SGLang 以及大多数 OpenAI 兼容服务器 | OpenAI Chat Completions |
| 提供 OpenAI `POST /v1/responses` 的服务器或代理 | OpenAI Responses |
| 提供 Anthropic `POST /v1/messages` 的服务器或代理 | Anthropic Messages |

- **同时提供嵌入：**当服务器以 OpenAI 的格式响应 `POST /v1/embeddings` 时开启。它的嵌入模型随后为 `/v1/embeddings` 提供服务。
- **支持 Token 计数：**只有 Responses 和 Messages 提供 Token 计数端点，即 `/v1/responses/input_tokens` 和 `/v1/messages/count_tokens`。仅当您的服务器能响应该端点时才保持开启。否则 Kaeso 会对其模型的计数请求返回 400：“The model `…` cannot count tokens.”

对于**其他服务器或代理**，Kaeso 使用原样的格式通信。它知道每个内置服务商与其格式的差异，例如 Mistral 拒绝哪些字段，但无法得知您的服务器的差异。您的服务器不接受的字段，会以您的服务器自己的错误返回。对于 Kaeso 了解的软件，它会像对内置服务商一样调整请求（[服务器软件](#服务器软件)）。格式无法承载的字段，Kaeso 会像对任何服务商一样拒绝（[Kaeso 拒绝的内容](../concepts/format-translation.md#kaeso-拒绝的内容)）。

## 服务器软件

Kaeso 知道下列推理服务器与其所用格式之间的差异。指定了**服务器**的自定义服务商会在每个请求中得到这些调整，控制台也会在该服务商出现的所有位置显示该软件的标志：

- `developer` 消息会作为 `system` 消息送达，因为模型的聊天模板通常只认识 `system`。Qwen 的模板会悄无声息地丢弃 `developer` 消息。
- 应用的输出上限以 `max_tokens` 发送，这些服务器都会读取它；Ollama 会忽略 `max_completion_tokens`。
- 终端用户的 ID 不会发送，Codex 的提示缓存键也不会发送，这些服务器不使用它们。
- 模型的思考内容，无论软件把它写在哪个字段，都能到达任何格式的应用：Claude Code 会把它显示为思考块，Kaeso 在后续轮次中也会以同一字段发回。
- 超出模型上下文的提示会以“提示过长”到达应用，因此 Claude Code 会压缩对话，[别名路由](../concepts/alias-routes.md)也可以回退到上下文更大的模型。Kaeso 认得 vLLM、SGLang、llama.cpp、LMDeploy 以及基于它们的服务器的这条消息。

| 服务器 | 识别依据 | 监听端口 | 说明 |
| --- | --- | --- | --- |
| Docker Model Runner | 所有者 `docker` | `12434`，位于 `/engines` 下 | 基础 URL 需带 `/engines`，例如 `http://host.docker.internal:12434/engines`。 |
| Jan | 所有者 `llama.cpp`、`mlx` 或 `remote` | `1337` | 在 Jan 的设置中开启本地 API 服务器；除 `127.0.0.1` 外的任何地址都需要密钥。 |
| KoboldCpp | 所有者 `koboldcpp` | `5001` | `--password` 设置密钥。 |
| llama.cpp | `server: llama.cpp` 响应头，所有者 `llamacpp` | 自 v0.7.0 起为 `9931`，llamafile 中为 `8080` | `--host 0.0.0.0` 供其他机器访问，`--api-key` 设置密钥。思考默认开启。 |
| LM Studio | 所有者 `organization_owner` | `1234` | 开启 **Serve on Local Network** 供其他机器访问，开启 **Require Authentication** 以使用密钥。 |
| LMDeploy | 所有者 `lmdeploy` | `23333` | `--api-keys` 设置密钥，`--reasoning-parser` 和 `--tool-call-parser` 开启思考和工具。 |
| LocalAI | `/version` | `8080` | `LOCALAI_API_KEY` 设置密钥。 |
| MLX LM | 无法识别 | `8080` | `mlx_lm.server` 只提供 Chat Completions。 |
| NVIDIA NIM | 无法识别 | `8000` | 请手动选择。 |
| Ollama | `/api/version` | `11434` | `OLLAMA_HOST=0.0.0.0` 供其他机器访问。Ollama 不使用密钥，请选择**不使用密钥**。Kaeso 会拒绝 Ollama 忽略的 `logit_bias`。 |
| SGLang | 所有者 `sglang` | `30000` | `--api-key` 设置密钥，`--reasoning-parser` 和 `--tool-call-parser` 开启思考和工具。 |
| TabbyAPI | 所有者 `tabbyAPI` | `5000` | 除非设置了 `disable_auth`，否则需要 `api_tokens.yml` 中的密钥。 |
| Text Generation Inference | `/info` | `3000`，其镜像中为 `80` | 只提供 Chat Completions，不含思考内容。Kaeso 会拒绝它忽略的 `logit_bias`。Hugging Face 已不再开发它。 |
| vLLM | 所有者 `vllm` | `8000` | `--api-key` 设置密钥，`--reasoning-parser` 开启思考，`--enable-auto-tool-choice --tool-call-parser <parser>` 开启工具。接受以 `video_url` 发送的视频。 |
| Xinference | 所有者 `xinference` | `9997` | 除非设置 `XINFERENCE_AUTH_ADVANCED=false`，否则需要密钥。 |

无法识别的服务器，或位于代理之后的服务器，可以在第一步或在**编辑**中选择其软件。选择其他软件只会改变 Kaeso 与服务器通信的方式，不会改变请求发往何处。

### 访问网关所在机器上的服务器

网关运行在容器中，在容器里 `localhost` 指的是容器本身。同一台机器上的服务器位于 `host.docker.internal`，例如 Ollama 为 `http://host.docker.internal:11434`，并且它必须监听回环地址以外的地址。Docker Desktop 认得这个名称；在 Linux 上，请在 `compose.override.yaml` 中为网关添加它：

```yaml title="compose.override.yaml"
services:
  gateway:
    extra_hosts:
      - host.docker.internal:host-gateway
    environment:
      KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS: 172.17.0.1/32
```

该地址属于私有网络，因此安装必须允许它（[Kaeso 可访问的地址](#kaeso-可访问的地址)）。下面的命令会输出 `host.docker.internal` 对网关而言的地址：

```bash
docker run --rm --add-host host.docker.internal:host-gateway alpine getent hosts host.docker.internal
```

把该地址加上 `/32` 填入 `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS`，然后用 `docker compose up -d --wait` 应用更改。

## Kaeso 无法得知的内容

KPMR 不了解您的服务器，而模型列表只给出模型的少数几项值。添加模型时，Kaeso 会填入它能找到的内容，其余由您提供：

- **价格。**输入和输出价格；如果服务器会报告缓存的 Token，还需填写缓存价格。Kaeso 从不自动填入价格，因为 KPMR 中的价格属于其他服务商。Kaeso 按这些价格对每个请求计费，并据此执行预算（[限额和预算](limits-and-budgets.md)）。对于不按 Token 付费的服务器，请填写 `0`。
- **限制。**上下文窗口和输出上限，取自模型列表，否则取自 KPMR 中的同名模型。Kaeso 将其用于预算，并用于补全 Messages 所要求的输出上限。
- **能力。**输入、输出、工具调用、推理、结构化输出、温度和推理强度等级，取自 KPMR 中的同名模型。请求需要模型不具备的能力时，Kaeso 会在调用您的服务器之前拒绝它。

这些值会保持您设置的样子，之后不会自动更新。

## 添加其模型

在项目的**模型**页面上选择**添加模型**。自定义服务商与内置服务商一起出现在服务商列表中。选择您的服务商后，对话框会使用项目的密钥查询服务器的模型列表 `GET /v1/models`，并列出服务器给出但目录中尚未包含的 ID。

选择一个 ID，或选择**输入模型 ID**，然后检查模型的各项值。Kaeso 会填入列表给出的限制，以及 KPMR 以相同名称收录的模型的各项值，比较时忽略大小写和 `/` 前的所有者：`meta-llama/Llama-3.3-70B-Instruct` 会采用 `meta-llama/llama-3.3-70b-instruct` 的值。价格始终由您填写。如果列表查询失败，对话框会说明原因，由您手动输入 ID。添加模型需要项目中的**管理模型**权限（[模型和别名](models-and-aliases.md)）。

之后，应用以模型名称调用该模型，名称前加上服务商的 ID：

```bash tab="curl"
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "research-vllm/llama-3.3-70b",
    "messages": [{"role": "user", "content": "Write a haiku about gateways."}]
  }'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)
completion = client.chat.completions.create(
    model="research-vllm/llama-3.3-70b",
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(completion.choices[0].message.content)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});
const completion = await client.chat.completions.create({
  model: "research-vllm/llama-3.3-70b",
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
console.log(completion.choices[0].message.content);
```

回复的 `kaeso-provider` 响应头、请求日志和用量都以 ID 标识该服务商。实例的指标将所有自定义服务商统计为 `custom`。

## Kaeso 可访问的地址

Kaeso 会在 DNS 解析之后检查每一次连接自定义服务商的地址，而不只是在添加时检查。它会拒绝私有、环回、链路本地和云元数据地址，例如 `10.0.0.5`、`127.0.0.1` 和 `169.254.169.254`，除非实例允许该网络。明文 `http` 只能访问实例允许的网络。Kaeso 不会跟随自定义服务商的重定向，并且不通过代理调用它。

添加位于此类地址的服务商会失败，提示“`base_url` 指向此实例不允许的地址。”如果某个地址之后不再被允许，发往它的请求会以 502 失败：“The custom provider `…` is at an address this install does not allow; ask its operator.”运维人员通过 `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS` 允许网络（[配置](../self-hosting/configuration.md#自定义服务商)）。

> [!WARNING]
> 能够管理自定义服务商的人，决定了 Kaeso 将服务商密钥和应用的提示词发往何处。请只把此权限授予您能放心交付这两者的人。

## 修改或删除自定义服务商

服务商的菜单中有**编辑**和**删除**。编辑可以修改 ID 之外的所有内容。新的基础 URL、密钥方式或格式会将其模型的[可用性](models-and-aliases.md#可用性)重置为**未检查**。要让服务商改为**不使用密钥**，请先删除它的密钥。

删除服务商也会删除它的服务商密钥，以及它在每个项目目录中的模型及其别名。此后对这些模型的请求都会失败。对话框会说明将删除多少个项目中的多少个密钥和模型，审计日志会保留被删除的内容。

## Kaeso 新增同 ID 服务商时

Kaeso 的后续版本可能会内置一个与您组织已用 ID 相同的服务商。在您的组织内，您的自定义服务商保留该 ID：它的密钥和请求仍然发往您的服务器，绝不会发给新的服务商。列表会将其标记为**替代**，并附上内置服务商的名称。

如果您想改用内置服务商，请删除您的自定义服务商，然后为内置服务商添加密钥和模型。

## 权限

| 操作 | 权限 |
| --- | --- |
| 查看自定义服务商及其基础 URL | **查看自定义服务商** |
| 添加、编辑和删除自定义服务商 | **管理自定义服务商**，需要近期登录 |

所有者和管理员拥有这两项权限，开发者和审计员可以查看。组织中的每个人都能在服务商列表中看到自定义服务商的名称，但看不到其基础 URL。如果实例关闭了自定义服务商，组织仍会保留它们，但对其模型的请求会失败，提示“Custom providers are turned off on this install, so `…` cannot be called.”

## 下一步

- [服务商密钥](provider-keys.md)：添加您的自定义服务商所需的密钥。
- [模型和别名](models-and-aliases.md)：将它的模型连同价格和限制添加到项目中。
- [配置](../self-hosting/configuration.md#自定义服务商)：允许自定义服务商访问的网络。
