---
title: KPMR-API
description: Die öffentlichen Endpunkte von KPMR, wie Sie den Katalog abrufen und aktuell halten und wie Sie seine Signatur prüfen.
order: 3
---

Der Katalog von KPMR und seine kleineren Dokumente stehen allen unter `https://kpmr.kaeso.app` offen, ohne Schlüssel und ohne Konto. Diese Anfrage ruft den Katalog zweier Anbieter ab und gibt die Eingabe- und Ausgabepreise von `claude-haiku-4-5` aus:

```bash tab="curl"
curl -s "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic,openai" \
  -H "Accept: application/vnd.kaeso.kpmr+json; version=1" \
  --compressed \
  | jq -r '.providers.anthropic.models["claude-haiku-4-5"].pricing
    | "\(.input) \(.output)"'
```

```python tab="Python"
import json
import urllib.request

request = urllib.request.Request(
    "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic,openai",
    headers={"Accept": "application/vnd.kaeso.kpmr+json; version=1"},
)
with urllib.request.urlopen(request) as response:
    catalogue = json.load(response)

models = catalogue["providers"]["anthropic"]["models"]
pricing = models["claude-haiku-4-5"]["pricing"]
print(pricing["input"], pricing["output"])
```

```ts tab="TypeScript"
const response = await fetch(
  "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic,openai",
  { headers: { Accept: "application/vnd.kaeso.kpmr+json; version=1" } },
);
const catalogue = await response.json();

const { pricing } = catalogue.providers.anthropic.models["claude-haiku-4-5"];
console.log(pricing.input, pricing.output);
```

Sie gibt die Preise in US-Dollar je Million Token aus:

```text
1 5
```

Das curl-Beispiel braucht `jq`. Das Python-Beispiel braucht nur die Standardbibliothek, und das TypeScript läuft im Browser oder in Node.js 18 und neuer. [Katalogformat](catalogue.md) erklärt jedes Feld der Antwort.

## Endpunkte

| Methode und Pfad | Antwortet mit |
| --- | --- |
| `GET /v1/catalogue` | Dem signierten Katalog. `?providers=` nennt die Anbieter, die Sie brauchen |
| `GET /v1/meta.json` | Revision, Format, Zahlen, den Anbietern, die Kaeso aufruft, in `kaeso_providers`, und den Signaturschlüsseln |
| `GET /v1/providers.json` | Jedem Anbieter mit der Zahl seiner Modelle und in `kaeso`, ob Kaeso ihn aufruft |
| `GET /v1/providers/<provider>.json` | Einem Anbieter: seinen Angaben, seinen Linien mit ihrem Verlauf, seinem Registry-Eintrag und einer Zusammenfassung jedes Modells |
| `GET /v1/models/<provider>/<model>.json` | Einem Modell: seinem Katalogeintrag, den Linien, die darauf zeigen, seinem Registry-Eintrag mit Quellen, angekündigten Preisen in `upcoming` und den Preisregeln, die gelten |
| `GET /v1/recent.json` | Den neuesten Modellen der Anbieter, die Kaeso aufruft. `?limit=` nimmt 1 bis 50, ohne es 12 |
| `GET /v1/deprecations.json` | Jeder kommenden Abschaltung aller Anbieter, die nächste zuerst |
| `GET /v1/search.json` | Dem Suchindex der Website: den Anbietern, die Kaeso aufruft, und ihren Modellen |
| `GET /v1/keys.json` | Den öffentlichen Schlüsseln, mit denen Kataloge signiert werden |
| `GET /healthz` | `status`, ob der Katalog bereit ist (`ready`), und seiner `revision` |

Nur der Katalog ist signiert. Die JSON-Dokumente erlauben jeden Origin, sodass eine Webseite sie lesen kann, und jeder Endpunkt beantwortet auch `HEAD` und `OPTIONS`. Eine Modell-ID behält ihr `/`, wie in `/v1/models/fal/alibaba/wan-3.0/text-to-video.json`; andere Zeichen kodieren Sie wie in jeder Adresse. Ein unbekannter Anbieter oder ein unbekanntes Modell antwortet `404` mit `error`.

## Der Katalog

### Abfrage

| Parameter | Bedeutung |
| --- | --- |
| `providers` | Bis zu 64 Anbieter-IDs, durch Kommas getrennt. Eine ID, die KPMR nicht kennt, fehlt in `providers`, statt abgelehnt zu werden, und `provider_ids` listet immer jede ID. Ohne den Parameter erhalten Sie jeden Anbieter, mehr als 10 MB vor der Kompression |

### Anfrage-Header

| Header | Bedeutung |
| --- | --- |
| `Accept` | `application/vnd.kaeso.kpmr+json; version=1`, die Formatversion, die Sie lesen, damit ein späteres Format Ihren Code nicht überrascht. `application/json` und kein `Accept` erhalten ebenfalls Format 1 |
| `If-None-Match` | Das `ETag` des Katalogs, den Sie haben. KPMR antwortet `304` ohne Body, solange er sich nicht geändert hat |
| `Accept-Encoding` | `br` oder `gzip` für eine komprimierte Antwort |
| `User-Agent` | Der Name Ihrer App, damit KPMR seine Leser unterscheiden kann |

### Antwort-Header

| Header | Bedeutung |
| --- | --- |
| `ETag` | Senden Sie es beim nächsten Mal in `If-None-Match` zurück |
| `KPMR-Revision` | Die `revision` des Katalogs |
| `KPMR-Signature` | `keyid=<key id>, ed25519=<signature>`, die Ed25519-Signatur des Bodys ([Signatur prüfen](#signatur-prüfen)) |
| `Cache-Control` | `public, max-age=60`; ein Cache darf eine veraltete Kopie ausliefern, während er neu fragt |

### Statuscodes

| Status | Bedeutung |
| --- | --- |
| `200` | Der Katalog, signiert und komprimiert, wenn Sie es annehmen |
| `304` | Unverändert seit dem `ETag`, das Sie gesendet haben: Behalten Sie, was Sie haben |
| `400` | `providers` ist leer, nennt mehr als 64 Anbieter oder enthält etwas, das keine Anbieter-ID ist. `error` sagt, was |
| `406` | Keine Formatversion, die Sie annehmen, etwa `version=2`. `supported` listet die, die KPMR ausliefert |
| `503` | KPMR startet noch. Versuchen Sie es nach `Retry-After` erneut |

Ein Fehler antwortet mit einem JSON-Body:

```json wrap
{ "error": "KPMR serves the catalogue as application/vnd.kaeso.kpmr+json in format 1.", "supported": [1] }
```

## Aktuell halten

Behalten Sie das `ETag` des Katalogs und sein `refresh_seconds`. Fragen Sie erneut, wenn so viele Sekunden vergangen sind, mit dem `ETag` in `If-None-Match`, und behalten Sie Ihren Katalog, solange KPMR `304` antwortet oder nicht erreichbar ist. Ein Kaeso-Gateway macht genau das ([Modelldaten aktuell halten](sync.md)).

## Signatur prüfen

KPMR signiert die genauen Bytes des Bodys vor der Kompression, also das, was Sie haben, wenn `gzip` oder Brotli aufgehoben ist. Prüfen Sie die Signatur, bevor Sie einer Kopie aus einem Spiegel oder Cache vertrauen, und hinterlegen Sie den öffentlichen Schlüssel fest in Ihrem Code, statt ihn neben dem Katalog abzurufen. `/v1/keys.json` listet die Schlüssel mit ihren IDs.

```python tab="Python"
import base64
import urllib.request

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

PUBLIC_KEY = "EQI7HVO5opUSzyPkqSLOOkOPFoYH+QywDUYDHaQUnd8="

request = urllib.request.Request(
    "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic",
    headers={"Accept": "application/vnd.kaeso.kpmr+json; version=1"},
)
with urllib.request.urlopen(request) as response:
    body = response.read()
    header = response.headers["KPMR-Signature"]

fields = dict(part.strip().split("=", 1) for part in header.split(","))
key = Ed25519PublicKey.from_public_bytes(base64.b64decode(PUBLIC_KEY))
key.verify(base64.b64decode(fields["ed25519"]), body)
print("Signed with", fields["keyid"])
```

```ts tab="TypeScript"
import { createPublicKey, verify } from "node:crypto";

const PUBLIC_KEY = "EQI7HVO5opUSzyPkqSLOOkOPFoYH+QywDUYDHaQUnd8=";

const key = createPublicKey({
  key: {
    kty: "OKP",
    crv: "Ed25519",
    x: Buffer.from(PUBLIC_KEY, "base64").toString("base64url"),
  },
  format: "jwk",
});

const response = await fetch(
  "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic",
  { headers: { Accept: "application/vnd.kaeso.kpmr+json; version=1" } },
);
const body = Buffer.from(await response.arrayBuffer());
const signature = response.headers.get("KPMR-Signature");
if (!signature) throw new Error("The catalogue isn't signed.");
const fields = Object.fromEntries(
  signature.split(",").map((part) => part.trim().split(/=(.*)/s)),
);

if (!verify(null, body, key, Buffer.from(fields.ed25519, "base64"))) {
  throw new Error("The catalogue's signature doesn't match.");
}
console.log("Signed with", fields.keyid);
```

Das Python-Beispiel braucht das Paket `cryptography` (`pip install cryptography`) und löst einen Fehler aus, wenn die Signatur nicht passt. Das TypeScript braucht Node.js 18 oder neuer.

## Abkündigungen

`/v1/deprecations.json` listet jedes Modell mit einer angekündigten Abschaltung, die noch bevorsteht, über alle Anbieter, die nächste zuerst. `kaeso` sagt, ob Kaeso den Anbieter aufruft:

```json wrap
[
  {
    "provider": "anthropic",
    "provider_name": "Anthropic",
    "kaeso": true,
    "id": "claude-sonnet-4-5-20250929",
    "name": "Claude Sonnet 4.5",
    "status": "deprecated",
    "lifecycle": {
      "announced_on": "2026-09-30",
      "deprecated_on": "2026-09-30",
      "shutdown_on": "2026-11-30",
      "replacement": "claude-sonnet-5-5",
      "source": "https://platform.claude.com/docs/en/about-claude/model-deprecations"
    }
  }
]
```

## Bedingungen

Der Katalog ist frei nutzbar, mit oder ohne Kaeso. Speichern Sie ihn zwischen und senden Sie `If-None-Match`, statt ihn oft abzurufen. Die Daten stehen unter der MIT-Lizenz: Geben Sie die Hinweise aus den [Lizenzen von KPMR](https://kpmr.kaeso.app/de/licenses) mit jeder Kopie weiter. Anbieter- und Modellnamen gehören ihren Inhabern.

## Nächste Schritte

- [Katalogformat](catalogue.md): Schlagen Sie jedes Feld des Katalogs nach.
- [Modelldaten aktuell halten](sync.md): Sehen Sie, wie ein Kaeso-Gateway den Katalog abruft und ihm vertraut.
- [Abschaltungen und Modelllinien](lifecycle-and-lines.md): Sehen Sie, wie sich die Daten des Lebenszyklus und die Linien ändern.
