---
title: 模型
description: 密钥可以调用的模型名称及其别名。
---

项目的目录列出其密钥可以调用的模型。

## 名称

调用模型时可以使用原生名称（`claude-haiku-4-5`）、`provider/model`（`anthropic/claude-haiku-4-5`）或别名。

```bash tab="curl"
curl https://kaeso.example.com/v1/models/fast -H "x-api-key: $KAESO_KEY"
```

```python tab="Python"
print(client.models.retrieve("fast"))
```

## 别名

别名是项目自己的名称，例如 `fast`，指向目录中的某个模型。

## 上下文窗口

窗口写作 `200K`、`1M` 或 `2M` 个令牌，`fast`、`claude-haiku-4-5` 和 `anthropic/claude-haiku-4-5` 共用最小的一个。

| 名称               | 窗口        |
| ------------------ | ----------- |
| `claude-haiku-4-5` | 200K 个令牌 |
| `fast`             | 200K 个令牌 |
