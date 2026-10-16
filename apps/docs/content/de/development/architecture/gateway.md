---
title: Gateway
description: Der Anfrageweg, die Konfiguration, die Modelldaten und die Regeln, denen der Rust-Code des Gateways folgt.
order: 1
---

Das Gateway ist der eine Server einer Installation: Apps rufen es mit den offiziellen SDKs von OpenAI und Anthropic auf, und die Konsole und Skripte verwalten es über die Platform API. Es ist der Rust-Workspace in `crates/`, gebaut zu einem einzigen Binary, `kaeso`. Was mit einer Anfrage von außen betrachtet geschieht, steht unter [Ablauf einer Anfrage](../../concepts/request-flow.md); diese Seite beschreibt das Innere.

## Der Anfrageweg

```text
app ─▶ /v1 ─▶ 1 key ─▶ 2 decode (KPP) ─▶ 3 model ─▶ 4 limits ─▶ 5 provider key ─▶ 6 encode (KPP) + send ─▶ provider
app ◀──────── 8 encode reply (KPP) ◀── 7 decode reply (KPP) ◀───────────────────────────────────────────── provider
                                               └─▶ 9 charge usage (Valkey, PostgreSQL) · 10 log (ClickHouse)

console, scripts ─▶ /platform/v1 ─▶ session cookie | API key with platform access | platform key
                                  ─▶ exactly one right per endpoint ─▶ service ─▶ PostgreSQL + audit event
```

Die Modell-API liegt in `crates/kaeso-server/src/model_api`, die Platform API in `crates/kaeso-server/src/platform` und die Services, die beide nutzen, in `crates/kaeso-server/src/services`. Die Crates sind unter [Architektur](index.md#der-rust-workspace) aufgeführt.

## Regeln

Jede Änderung am Gateway folgt diesen Regeln; die meisten davon erzwingen Lints oder Tests.

1. **Wire-Formate gibt es nur in KPP.** Das Gateway parst oder baut nie von Hand das JSON von Anfragen, Antworten, Streams, Fehlern oder Modelllisten von OpenAI oder Anthropic; das übernimmt `kaeso_codecs::dispatch`. Das eigene JSON des Gateways sind die Platform API und ihr Fehler-Envelope.
2. **Kein Passthrough.** Jede Modellanfrage wird nach KPP dekodiert und wieder kodiert, auch wenn App und Anbieter dasselbe Format sprechen.
3. **Keine Wiederholungen und keine Fallbacks.** Ein fehlgeschlagener Aufruf beim Anbieter schlägt fehl, im Format der App.
4. **Nie stillschweigend Daten verwerfen.** Was KPP ablehnt (`Unsupported`), geht als Fehler im Format der App an sie zurück, bevor irgendetwas an einen Anbieter gesendet wird. Ein `KppError` wird nie abgefangen und ignoriert.
5. **Keine Panics in Library-Crates.** Kein `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, keine panikende Indizierung und kein `unsafe` außerhalb von Tests und dem Start des Binarys; die Workspace-Lints in `Cargo.toml` und `#![forbid(unsafe_code)]` erzwingen das.
6. **Geheimnisse verlassen den Speicher nie ungeschützt.** Anbieter-Schlüssel, Master-Keys, Geheimnisse von API-Schlüsseln, Sitzungs-IDs, Anmelde-Tokens, TOTP-Geheimnisse und Passwörter werden nie protokolliert, in Traces geschrieben, ausgegeben, in ClickHouse gespeichert, von einer API zurückgegeben oder in eine Fehlermeldung geschrieben. Sie leben in schwärzenden Wrapper-Typen.
7. **Geld ist nie ein Float.** `rust_decimal::Decimal` in Rust, `NUMERIC` in PostgreSQL, `Decimal` in ClickHouse; die laufenden Ausgabenzähler in Valkey sind ganzzahlige Nano-Dollar, aufgerundet.
8. **Jede Änderung über die Platform API wird auditiert, und jeder Endpunkt prüft genau ein Recht** aus dem Katalog in `crates/kaeso-core/src/rights/catalogue.rs` ([Rechte](../../concepts/rights.md)), außer `me/*` und `auth/*`, und das Instanz-Admin-Flag für `instance/*`. Routen werden mit ihrem Recht registriert, sodass eine fehlende Prüfung einen Test scheitern lässt.
9. **Organisationen sind voneinander isoliert.** Jede Abfrage an PostgreSQL und ClickHouse, die Daten einer Organisation liest, filtert nach der Organisation aus dem authentifizierten Kontext, nie allein nach Benutzereingaben.
10. **Migrationen laufen nur vorwärts** und werden beim Start unter einer Sperre ausgeführt. Sie liegen in `crates/kaeso-store/migrations/postgres` (sqlx) und `crates/kaeso-store/migrations/clickhouse` (geordnete SQL-Dateien). Eine Migration, die einen manuellen Schritt braucht, macht das Release zu einem Major-Release.
11. **Das OpenAPI-Dokument wird generiert.** Zusammenfassungen und Beschreibungen von Operationen stehen in `#[utoipa::path(summary = …, description = …)]`, Typbeschreibungen in `#[schema(description = …)]` und Beschreibungen von Feldern, Varianten und Query-Parametern in `#[doc = "…"]`-Attributen, der einzigen Form, die utoipa dort liest. `cargo run -q -p kaeso -- openapi > openapi.json` schreibt das Dokument neu.
12. **Tests sind deterministisch.** Uhren und Zufallsquellen werden injiziert, und nichts schläft, um sich zu synchronisieren; Tests pollen mit Fristen.

## Konfiguration

Jede Einstellung ist eine Zeile der Tabelle in `crates/kaeso-server/src/config/settings.rs`: ihr TOML-Schlüssel, ihre Umgebungsvariable, ihr Typ, ihr Standardwert, ob sie ein Geheimnis ist, und eine Beschreibung in einem Satz. Das Laden führt die Standardwerte, die TOML-Datei aus `KAESO_CONFIG` und die `KAESO_*`-Variablen zusammen, einschließlich der `_FILE`-Variablen für Geheimnisse, in `config/load.rs` über das Crate `toml` und mit Fehlern, die immer die Einstellung nennen.

Tests prüfen, dass jeder Umgebungsname seinem Schlüssel folgt, dass Pflichteinstellungen und Geheimnisse keinen Standardwert haben, dass jede Beschreibung ein Satz ist und dass `deploy/kaeso.example.toml` jede Einstellung außer den Geheimnissen auf ihren Standardwert setzt und sich laden lässt, und sie rendern die Tabelle in einen geprüften Snapshot (`crates/kaeso-server/tests/snapshots/`). Eine Änderung an einer Einstellung ändert diesen Snapshot, und [Konfiguration](../../self-hosting/configuration.md) wird danach aktualisiert. Ein Konfigurations-Crate wie figment brächte eine zweite Quelle der Wahrheit für Schlüssel und Standardwerte mit, deshalb gibt es keines.

## Modelldaten

`crates/kaeso-core/data` enthält die Modelldaten, die mit jedem Release ausgeliefert werden:

| Datei | Inhalt |
| --- | --- |
| `models-dev.json` | Ein Snapshot von [models.dev](https://models.dev), gefiltert auf die Anbieter, die Kaeso aufruft, sodass eine frische Installation ohne Netzwerkzugriff funktioniert. Die `README.md` des Ordners hält fest, wann er abgerufen wurde, und seinen SHA-256. |
| `supplement.toml` | Felder, die models.dev nicht hat, mit der Quelle jedes Eintrags in der `README.md` des Ordners: `adaptive_thinking` für Anthropic-Modelle, `efforts`, wo models.dev keine angibt, und `egress_dialect`, das Format, in dem Kaeso an ein OpenAI-Modell sendet. |

Der Wert eines Katalogeintrags ist sein Override, sonst models.dev, sonst das Supplement, sonst leer. Beim Synchronisieren, standardmäßig alle 24 Stunden, wendet das Gateway denselben Filter an und speichert das Ergebnis in PostgreSQL; beim Start gewinnt der neuere von gespeichertem und eingebautem Snapshot. Nur die Anbieter in `crates/kaeso-core/src/provider.rs` bleiben erhalten.

Vor einem Release den Snapshot aktualisieren:

```bash
cargo xtask models-dev
```

Der Befehl lädt `https://models.dev/api.json` herunter (`--url` oder `MODELS_DEV_URL` ändert die Adresse), filtert es, prüft, dass das Gateway das Ergebnis lesen kann, schreibt den Snapshot, die Tabelle in `crates/kaeso-core/data/README.md` und den Abrufzeitpunkt in `crates/kaeso-core/src/models_dev.rs` und gibt den neuen Hash aus. Tests prüfen, dass die drei übereinstimmen.

## Fehlermeldungen

Die Konsole zeigt die `message` der Platform API unverändert in einem Toast an, daher ist jede Meldung ein kurzer Satz:

- Höchstens 90 Zeichen so, wie sie im Code steht, Platzhalter eingeschlossen, und höchstens 120, wenn sie ausgefüllt ist. Sie endet mit einem Punkt.
- Ein Satz: zwei Gedanken mit einem Semikolon verbinden (`Your session has ended; sign in again.`) oder den zweiten weglassen. Keine Abkürzungen wie „e.g.“, keine Zeilenumbrüche.
- Schlicht und konkret: sagen, was falsch ist, und, wo es passt, was zu tun ist. Felder als Code-Spans nennen (`` `amount` must be positive. ``) und das Feld in `param` setzen.
- Keine internen Namen (ClickHouse, Valkey, Konfigurationsschlüssel, Modulpfade) und keine angehängten technischen Details wie Fehler von Anbietern, SMTP oder serde: Diese mit `tracing` protokollieren und die Meldung kurz halten.
- Interne Ursachen (`ApiError::internal`, das `#[error]` interner Typen, Logzeilen) beginnen mit einem Kleinbuchstaben; Meldungen für Personen beginnen mit einem Großbuchstaben oder einem Code-Span.

`crates/kaeso-server/tests/error_copy.rs` durchsucht den Code hinter der Platform API und schlägt bei jeder Meldung fehl, die diese Regeln verletzt, und die Integrationstests prüfen jede Antwort der Platform API, die sie erhalten (`crates/kaeso-server/tests/common/copy.rs`). Die Modell-API behält die Fehlerformate ihrer Anbieter bei, daher gelten diese Regeln für ihre Meldungen nicht.
