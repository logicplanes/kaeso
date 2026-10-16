---
title: Schlüssel und Limits
description: Einen Schlüssel für eine App erstellen und ihre Ausgaben in einem Budget halten.
order: 1
---

# Schlüssel und Limits

Jede Anfrage an das Gateway trägt einen **Schlüssel**. Ein Schlüssel gehört einem Mitglied eines Projekts, handelt mit dessen Rechten und kann eigene _Limits_ haben. Ersetzte Schlüssel bleiben als ~~aktive~~ archivierte Schlüssel lesbar.

Verwenden Sie `sk-kso-…`-Schlüssel nur auf Servern; welche Namen ein Schlüssel aufrufen darf, steht unter [Modelle](../reference/models.md#aliasse), mehr zu [Ratenlimits](#ratenlimits) weiter unten.

## Schlüssel erstellen

1. Das Projekt öffnen und **API-Schlüssel** wählen.
2. **Schlüssel erstellen** wählen und ihn nach der App benennen, die ihn nutzt.
   - Eine Laufzeit wählen: 30, 90 oder 365 Tage.
   - **Modelle** leer lassen, um jedes Modell des Projekts zu erlauben.
3. Das Geheimnis kopieren: Es wird nur einmal gezeigt.

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 100, "messages": [{"role": "user", "content": "Hallo"}]}'
```

> [!NOTE]
> Das Geheimnis eines Schlüssels wird nie lesbar gespeichert. Geht es verloren, einen neuen Schlüssel erstellen und den alten archivieren.

### Aus einem SDK

Die offiziellen SDKs funktionieren unverändert; nur die Basis-URL und der Schlüssel ändern sich.

```python
from anthropic import Anthropic

claude = Anthropic(base_url="https://kaeso.example.com", api_key="sk-kso-…")
claude.messages.create(model="claude-haiku-4-5", max_tokens=100, messages=[{"role": "user", "content": "Hallo"}])
```

```ts title="support-bot/src/client.ts"
import OpenAI from "openai";

export const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_KEY });
```

## Limits

Limits gelten auf drei Ebenen, und eine Anfrage muss in alle passen.

| Ebene    | Anfragen pro Minute | Token pro Tag | Ausgaben pro Monat | Gilt für                                     |
| -------- | ------------------: | ------------: | -----------------: | -------------------------------------------- |
| Projekt  |               1.200 |    40.000.000 |         2.500,00 $ | Jedes Mitglied und jeden Schlüssel           |
| Mitglied |                 300 |    10.000.000 |           500,00 $ | Jeden Schlüssel des Mitglieds im Projekt     |
| Schlüssel |                 60 |     1.000.000 |            50,00 $ | Nur Anfragen mit diesem Schlüssel            |

> [!TIP]
> Jeder App einen eigenen Schlüssel geben: Dann gehören ihr Nutzung und Limits allein.

### Ratenlimits

Eine Anfrage über einem Ratenlimit wird abgelehnt, bevor sie den Anbieter erreicht, mit `429` und einem `retry-after`-Header.

```json
{
  "type": "error",
  "error": { "type": "rate_limit_error", "message": "The key's limit of 60 requests per minute is reached; try again in 12 seconds." }
}
```

> [!IMPORTANT]
> Limits zählen die Anfragen, die das Gateway angenommen hat, gestreamte eingeschlossen.

### Budgets

Budgets werden vor dem Senden geprüft, mit dem Preis des Modells zum Zeitpunkt der Anfrage.

> [!WARNING]
> Ein Budget, das Mitte des Monats erreicht ist, lehnt jede Anfrage des Schlüssels ab, bis der Monat endet oder das Budget wächst.

> [!CAUTION]
> Das Archivieren eines Schlüssels lässt sich nicht rückgängig machen: Apps, die ihn nutzen, stoppen sofort.

## Checkliste

- [x] Für jede App einen Schlüssel erstellen
- [x] Eine Laufzeit festlegen
- [ ] Ein Monatsbudget hinzufügen
- [ ] Nach einer Woche die Anfrageprotokolle prüfen

## Hinweise

> Schlüssel sind billig; geteilte Schlüssel sind teuer zu entwirren.
>
> — aus der Betriebsanleitung

Der Weg einer Anfrage, von der App zum Anbieter:

![Der Weg einer Anfrage: App, Schlüssel, Modell, Limits, Anbieter](images/request-flow.svg)

Eine lange Adresse bleibt in der Spalte: https://kaeso.example.com/platform/v1/orgs/org_01jb2xk7e9r3q5w8v4n6m2p0zt/projects/prj_01jb2xk7e9r3q5w8v4n6m2p0zt/keys.

Eine Sprache ohne Hervorhebung erscheint als einfacher Text:

```toml
[gateway]
public_url = "https://kaeso.example.com"
```

---

Als Nächstes einen Anbieter verbinden: [Anbieter-Schlüssel](provider-keys.md).
