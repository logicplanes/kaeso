---
title: Mehrere Gateways
description: Betreiben Sie mehr als ein Gateway hinter Caddy oder einem Load Balancer und wissen Sie, was sie teilen.
order: 13
---

Ein Gateway bedient Tausende Verbindungen. Reicht eines nicht, oder soll eines weiter bedienen, während ein anderes neu startet, betreiben Sie mehrere. Sie teilen alles, wovon eine Anfrage abhängt, jedes Gateway kann also jede Anfrage beantworten.

## Was Gateways teilen

Jedes Gateway einer Installation liest dasselbe PostgreSQL, Valkey und ClickHouse und behält nichts für sich, was eine Anfrage braucht:

| Geteilt über | Was |
| --- | --- |
| PostgreSQL | Benutzer, Sitzungen, Organisationen, Projekte, Schlüssel, Kataloge, Limits, Nutzung und Audit-Logs; eine Sitzung, die sich an einem Gateway angemeldet hat, gilt an jedem anderen |
| Valkey | Raten-, Token- und Budgetzähler, ein Limit gilt also über alle Gateways hinweg; der [Antwort-Cache](../console/response-cache.md); Reasoning-Kontinuität; die geteilten Caches für Schlüssel, Kataloge und Einstellungen, deren Invalidierungen jedes Gateway innerhalb einer Sekunde erhält; welche Schritte von [Alias-Routen](../concepts/alias-routes.md#übersprungene-schritte) gerade scheiterten, sodass der gescheiterte Aufruf eines Gateways den anderen erspart bleibt |
| PostgreSQL-Sperren | Hintergrundjobs: KPMR-Sync, Update-Prüfungen, Benachrichtigungen und Aufräumarbeiten laufen auf jeweils einem Gateway, nie einmal je Gateway |

Jedes Gateway behält nur für sich, was zu seinem Prozess gehört: die Anfrageprotokoll-Zeilen, die es jede Sekunde schreibt, und die Nutzung, die es alle 10 Sekunden festhält, sein Speicherbudget für das Übersetzen von Streams und seine Metriken. Fragen Sie `/metrics` jedes Gateways ab; die Zähler addieren sich ([Zustand und Metriken](monitoring.md#metriken)).

## Im Compose-Paket

Legen Sie die Zahl der Gateways in der `.env` fest und wenden Sie sie an:

```bash
echo GATEWAY_REPLICAS=2 >> .env
docker compose up -d --wait
```

Compose startet `gateway-1`, `gateway-2` und so weiter, jedes mit eigenen Speicher- und Prozessgrenzen ([Container-Grenzen](configuration.md#container-grenzen)). Caddy löst `gateway` jede Sekunde über Dockers DNS auf, sendet jede Anfrage an das Gateway mit den wenigsten offenen Verbindungen und lässt eines, das scheitert, 5 Sekunden lang aus. `docker compose exec gateway` erreicht das erste, was für `kaeso setup-link` und die anderen Befehle genügt.

Der Updater behandelt jedes Replikat: Er stoppt alle, startet dann die neuen nacheinander und wartet, bis jedes gesund ist, damit kein altes Gateway gegen eine Datenbank läuft, die das neue gerade migriert ([der Updater](upgrades.md#der-updater)).

## Was es bringt

Gemessen am 7. November 2026 mit Docker 29.5 auf einem Apple-M-Mac, jedes Gateway auf eine CPU begrenzt, Caddy davor, der Mock-Anbieter dahinter und 64 Verbindungen, die 20 Sekunden lang Chat Completions senden: nicht gestreamt in drei Läufen (der mittlere), gestreamt in zwei (der niedrigere):

| Gateways | Anfragen pro Sekunde | Median | 99. Perzentil | Streams pro Sekunde | Median der Streams |
| --- | --- | --- | --- | --- | --- |
| 1 | 7.951 | 7,7 ms | 16,6 ms | 2.614 | 23,9 ms |
| 2 | 11.910 | 5,1 ms | 11,2 ms | 4.561 | 13,8 ms |
| 3 | 16.143 | 3,6 ms | 9,5 ms | 7.355 | 8,3 ms |

Drei Gateways bedienen doppelt so viele Completions wie eines bei halber Latenz und das 2,8-Fache der Streams, deren Übersetzung bei jedem Ereignis CPU kostet. Completions gewinnen weniger als die Zahl der Gateways, weil Caddy, der Mock-Anbieter und der Lastgenerator dieselbe Maschine teilten; auf getrennten Hosts oder mit je einer CPU bleibt der Gewinn nahe daran. Lange Streams, die auf einen echten Anbieter warten, gewinnen ebenfalls Kapazität: Jedes Gateway hat sein eigenes `KAESO_SERVER_MAX_CONNECTIONS` ([Konfiguration](configuration.md#einstellungen)).

Ein Gateway, das neu startet, für ein Update oder nach einem Absturz, kostet nichts, während die anderen bedienen. Caddy schickt ihm innerhalb von 5 Sekunden nichts mehr und kehrt zurück, sobald es gesund ist.

## Auf einer verwalteten Plattform

Die [Plattformvorlagen](platforms/index.md) nehmen die Zahl der Gateways als `GatewayCount` auf AWS und `gateway_min_instances` auf Google Cloud und `instance_count` auf DigitalOcean entgegen; Render und Railway skalieren einen Dienst aus ihren Dashboards. Der Load Balancer davor braucht nichts Klebriges, denn nichts, wovon eine Anfrage abhängt, liegt auf einem einzelnen Gateway.

## Nächste Schritte

- [Konfiguration](configuration.md): `GATEWAY_REPLICAS` und die Grenzen, die jedes Gateway bekommt.
- [Zustand und Metriken](monitoring.md): Fragen Sie jedes Gateway ab und lesen Sie die Zähler, die sich addieren.
- [Upgrades](upgrades.md): Wie der Updater jedes Replikat ersetzt.
