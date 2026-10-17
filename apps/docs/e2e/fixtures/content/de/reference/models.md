---
title: Modelle
description: Die Modellnamen, die ein Schlüssel aufrufen darf, und ihre Aliasse.
---

Der Katalog eines Projekts listet die Modelle, die seine Schlüssel aufrufen dürfen.

## Namen

Ein Modell wird mit seinem nativen Namen (`claude-haiku-4-5`), mit `provider/model` (`anthropic/claude-haiku-4-5`) oder mit einem Alias aufgerufen.

## Aliasse

Ein Alias ist ein eigener Name des Projekts, etwa `fast`, der auf ein Modell seines Katalogs zeigt.

## Kontextfenster

Ein Fenster wird als `200K`, `1M` oder `2M` Tokens geschrieben, und `fast`, `claude-haiku-4-5` sowie `anthropic/claude-haiku-4-5` teilen sich das kleinste.

| Name               | Fenster     |
| ------------------ | ----------- |
| `claude-haiku-4-5` | 200K Tokens |
| `fast`             | 200K Tokens |
