---
title: Models
description: The model names a key may call, and their aliases.
---

A project's catalogue lists the models its keys may call.

## Names

A model is called by its native name (`claude-haiku-4-5`), by `provider/model` (`anthropic/claude-haiku-4-5`), or by an alias.

```bash tab="curl"
curl https://kaeso.example.com/v1/models/fast -H "x-api-key: $KAESO_KEY"
```

```python tab="Python"
print(client.models.retrieve("fast"))
```

## Aliases

An alias is a name of the project's own, such as `fast`, that points at a model of its catalogue.

## Context windows

A window is written as `200K`, `1M`, or `2M` tokens, and `fast`, `claude-haiku-4-5`, and `anthropic/claude-haiku-4-5` share the smallest.

| Name               | Window      |
| ------------------ | ----------- |
| `claude-haiku-4-5` | 200K tokens |
| `fast`             | 200K tokens |
