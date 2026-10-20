---
title: 服务商密钥
description: 连接组织的模型服务商密钥，并选择各项目使用哪个密钥。
order: 3
---

服务商密钥是组织用于 Kaeso 所调用的模型服务商的密钥。Kaeso 使用实例的主密钥加密存储它们，添加后不再显示。应用永远看不到这些密钥，而是使用 [API 密钥](api-keys.md)调用 Kaeso。

## 添加密钥

**服务商**页面列出组织已有密钥的服务商。在组织添加第一个密钥之前，该页面会介绍服务商密钥，并提供**添加服务商密钥**。

1. 在**服务商**页面上选择**添加密钥**。
2. **此密钥属于哪个服务商？** 搜索并选择服务商。已有密钥的服务商排在前面，并显示其密钥数量。
3. 粘贴密钥并为其命名，例如 `Production`。对话框会链接到服务商创建密钥的页面，下表也列出了这些页面。
4. 如果该服务商已有默认密钥，选择新密钥是否成为默认密钥。

| 服务商 | 在此创建密钥 | 密钥开头 |
| --- | --- | --- |
| OpenAI | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) | `sk-` |
| Anthropic | [platform.claude.com/settings/keys](https://platform.claude.com/settings/keys) | `sk-ant-` |
| DeepSeek | [platform.deepseek.com/api_keys](https://platform.deepseek.com/api_keys) | `sk-` |
| SpaceX | [console.x.ai](https://console.x.ai)（xAI 控制台）的 **API Keys** | `xai-` |
| Together AI | [api.together.ai](https://api.together.ai/settings/projects/~current/api-keys) 的 **API Keys** | `tgp_v1_` |
| Groq | [console.groq.com/keys](https://console.groq.com/keys) | `gsk_` |
| OpenRouter | [openrouter.ai/settings/keys](https://openrouter.ai/settings/keys) | `sk-or-v1-` |

OpenRouter 将请求转发给众多服务商的模型。它的模型 ID 包含模型厂商，例如 `anthropic/claude-sonnet-4.5`。请以 `openrouter/anthropic/claude-sonnet-4.5` 的形式调用，这样请求不会发往该厂商自己的服务商；参见[请求如何指定模型](models-and-aliases.md#请求如何指定模型)。Kaeso 按目录中的价格计算 OpenRouter 请求的费用，而不是按 OpenRouter 在 `usage.cost` 中报告的费用。OpenRouter 列出模型时不检查密钥，因此**检查可用性**无法发现错误的 OpenRouter 密钥；使用该密钥的第一个请求会报告密钥被拒绝。

对服务商密钥的每种操作都需要相应的权限：

| 操作 | 权限 |
| --- | --- |
| 添加密钥 | **添加服务商密钥** |
| 替换、重命名、分配密钥或将其设为默认 | **更改服务商密钥** |
| 删除密钥 | **删除服务商密钥** |

除删除外，每项操作还需要近期登录。

## 选择项目使用的密钥

每个服务商有一个默认密钥，所有未分配密钥的项目都使用它。为服务商添加的第一个密钥会成为默认密钥。要更换默认密钥，在另一个密钥的菜单中选择**设为默认**。

要让部分项目使用其他密钥，在该密钥的菜单中选择**分配给项目**并选择项目。这些项目随后会用该密钥而非默认密钥调用该服务商。每个项目对每个服务商只使用一个密钥。

## 替换、重命名或删除密钥

列表显示每个密钥的名称、最后四个字符、使用它的项目、添加者以及上次使用时间。密钥菜单提供以下操作：

| 操作 | 作用 |
| --- | --- |
| 替换 | 为同一个密钥换上新的密钥值，例如在服务商处轮换之后 |
| 重命名 | 更改控制台中显示的名称 |
| 删除 | 移除密钥。其项目改回使用默认密钥 |

删除默认密钥时，该服务商最近添加的另一个密钥会成为默认密钥。如果没有其他密钥，对该服务商的请求将失败。对话框会说明具体情况。

> [!NOTE]
> 项目使用的密钥决定了服务商为其提供哪些模型。添加、替换、分配、删除密钥或将其设为默认，都会把该服务商的模型在所有项目中的[可用性](models-and-aliases.md#可用性)重置为**未检查**。

## 后续步骤

- [模型和别名](models-and-aliases.md)：添加项目可调用的模型。
- [API 密钥](api-keys.md)：创建应用调用 Kaeso 所用的密钥。
