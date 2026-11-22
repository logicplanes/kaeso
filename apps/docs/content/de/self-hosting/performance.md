---
title: Leistung
description: Sehen Sie, wie viel Zeit, CPU und Speicher Kaeso neben LiteLLM, Bifrost und Portkey in BerriAIs Benchmark hinzufügt.
order: 14
---

Kaeso 0.7.0 lief in [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench), dem Benchmark von BerriAI, der Firma hinter LiteLLM. LiteLLM, Bifrost und Portkey liefen daneben auf derselben Maschine. Nutzen Sie die Zahlen, um einen Server zu bemessen und Kaeso mit dem Gateway zu vergleichen, das Sie heute betreiben. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) zeigt sie als Diagramme, mit den Skripten und allen Rohdaten.

Jede Zahl ist der Median aus drei Runden. Der Mock-Anbieter des Benchmarks steht für den Modellanbieter, daher zeigen die Zahlen die Kosten des Gateways selbst, nicht die eines Modells.

## Kapazität

Diese Zahlen stammen von 64 Clients gegen einen Mock, der sofort antwortet, jedes Gateway auf denselben zwei dedizierten Kernen. Derselbe Treiber schaffte direkt zum Mock etwa 28.000 Anfragen pro Sekunde, also lief jedes Gateway an seiner eigenen Grenze:

| Gateway | Anfragen/s | Latenz p50 / p99 | CPU pro Anfrage | Höchster Speicher |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.0 | 6.338 | 10,0 / 15,6 ms | 314 µs | 101 + 271 MB |
| Kaeso 0.7.0, Anfrageprotokolle an | 5.324 | 11,8 / 20,6 ms | 373 µs | 450 + 1.163 MB |
| LiteLLM Rust, `main` bei `d4e8f96` | 3.211 | 19,6 / 33,2 ms | 573 µs | 86 MB |
| Bifrost 2.6.0 | 2.806 | 21,0 / 54,6 ms | 708 µs | 405 MB |
| Portkey 1.15.2 | 779 | 79,9 / 118,3 ms | 1.449 µs | 293 MB |
| LiteLLM Python 1.110.2 | 74 | 858,1 / 974,0 ms | 13.563 µs | 743 MB |

Die CPU pro Anfrage ist die des Gateways. Kaesos Speicher ist der des Gateways plus der seiner Datenbanken: PostgreSQL und Valkey, mit Anfrageprotokollen auch ClickHouse. Die anderen Gateways liefen ohne Datenbank. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) zeigt die Spanne jeder Runde.

## Zusätzliche Latenz

Zusätzliche Latenz ist die Zeit einer Anfrage durch das Gateway minus die Zeit derselben Anfrage direkt zum Mock. Der Mock antwortet wie ein schneller Anbieter in etwa 22 ms. Jede Zelle zeigt p50 / p99:

| Gateway | 1 Client | 4 Clients | 16 Clients |
| --- | --- | --- | --- |
| Kaeso 0.7.0 | +0,73 / +1,66 ms | +0,56 / +1,15 ms | +0,54 / +1,47 ms |
| Kaeso 0.7.0, Anfrageprotokolle an | +0,77 / +2,22 ms | +0,60 / +1,90 ms | +0,62 / +1,45 ms |
| LiteLLM Rust, `main` bei `d4e8f96` | +0,87 / +0,98 ms | +0,76 / +1,68 ms | +0,69 / +2,26 ms |
| Bifrost 2.6.0 | +1,10 / +1,32 ms | +0,85 / +2,41 ms | +0,71 / +4,08 ms |
| Portkey 1.15.2 | +2,04 / +5,26 ms | +1,17 / +6,99 ms | +3,1 / +20,4 ms |
| LiteLLM Python 1.110.2 | +16,0 / +17,8 ms | +50,9 / +60,5 ms | +188 / +216 ms |

Eine Zeile für 64 Clients fehlt. Bei dieser Last teilen sich Mock und Treiber zwei Kerne und kommen nicht über etwa 2.900 Anfragen pro Sekunde. Den direkten Lauf abzuziehen, würde dann die Zeit jedes Gateways zu klein ausweisen; diese Last misst stattdessen die Kapazitätstabelle.

## Streams und Agent-Sitzungen

Der Streaming-Lauf sendet 200 gestreamte Anfragen bei 16 Clients. Die Sitzungsläufe spielen eine aufgezeichnete Claude-Code-Sitzung und eine aufgezeichnete Codex-Sitzung ab, je 30 Turns. Die zusätzliche Zeit einer Sitzung ist ihre Gesamtzeit durch das Gateway minus direkt zum Mock:

| Gateway | Zusätzliche Zeit bis zum ersten Token | Chunk-Abstand p99 | Claude-Code-Sitzung | Codex-Sitzung |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.0 | +0,87 ms | 7,8 ms | +41,5 ms | +34,0 ms |
| Kaeso 0.7.0, Anfrageprotokolle an | +0,94 ms | 8,1 ms | +66,0 ms | +65,5 ms |
| LiteLLM Rust, `main` bei `d4e8f96` | +0,96 ms | 40,4 ms | +41,0 ms | +44,2 ms |
| Bifrost 2.6.0 | Nicht vergleichbar | 20,5 ms | +126,4 ms | +91,3 ms |
| Portkey 1.15.2 | +27,54 ms | 8,6 ms | +91,4 ms | +104,2 ms |
| LiteLLM Python 1.110.2 | +118,92 ms | 34,0 ms | +502,1 ms | +507,7 ms |

Direkt zum Mock liegt der Chunk-Abstand bei 7,5 ms im p99. Bifrost sendet sein erstes Ereignis, bevor der Anbieter antwortet, und der Benchmark zählt das als erstes Token. Deshalb ist seine Zeit bis zum ersten Token negativ. Jede gestreamte Sitzung lieferte durch jedes Gateway gültige Tool-Aufrufe.

## Wo andere Gateways vorn liegen

- **Speicher:** LiteLLM Rust kam nie über 86 MB und braucht keine Datenbank. Kaeso erreichte 164 MB, und seine PostgreSQL und Valkey belegten weitere 271 MB.
- **Ein Client:** Mit einer Anfrage zur Zeit lag das p99 von LiteLLM Rust bei +0,98 ms, das von Kaeso bei +1,66 ms, und keine Runde von Kaeso kam näher.
- **Gleichstand:** Die Zeit bis zum ersten Token und die Claude-Code-Sitzung liegen in der Spanne des jeweils anderen.
- **Anfrageprotokolle:** Mit jeder Anfrage in ClickHouse schaffte Kaeso 16 % weniger Anfragen pro Sekunde, und seine Datenbanken belegten 1.163 MB.

## Was jedes Gateway tat

Jedes Gateway lief so, wie sein Projekt es ausliefert, gegen den Mock im Format Anthropic Messages:

| Gateway | Bei jeder Anfrage geprüft | Gespeichert |
| --- | --- | --- |
| Kaeso 0.7.0 | Den [API-Schlüssel](../console/api-keys.md) eines Projekts und die Modelle, die das Projekt erlaubt | Nutzung in PostgreSQL; keine Anfrageprotokolle |
| Kaeso 0.7.0, Anfrageprotokolle an | Dasselbe | Nutzung in PostgreSQL und jede Anfrage in ClickHouse |
| LiteLLM Rust, `main` bei `d4e8f96`, unveröffentlicht | Einen Master-Schlüssel aus seiner Konfiguration | Nichts; keine Datenbank |
| LiteLLM Python 1.110.2 | Einen Master-Schlüssel aus seiner Konfiguration | Nichts; keine Datenbank |
| Bifrost 2.6.0 | Keinen Schlüssel | Nichts; Logging und Stores aus |
| Portkey 1.15.2 | Keinen Schlüssel | Nichts; Logging aus |

## Wie es lief

- **Maschine:** eine DigitalOcean CPU-Optimized c-4 mit 4 dedizierten vCPUs (Intel Xeon Platinum 8358, 2,6 GHz), 8 GB Speicher, Ubuntu 24.04 und Docker, am 22. November 2026.
- **Kerne:** Jedes Gateway lief in Docker im Host-Netzwerk, fest auf zwei Kernen. Mock, Lasttreiber und Kaesos Datenbanken liefen auf den anderen beiden.
- **Benchmark:** `BerriAI/ai-gateway-bench` bei `44365ca`, Mock-Anbieter und Treiber unverändert: `load-probe` für Anfragen, `run_http_bench.py` für Streams und `session_runner.py` für Sitzungen.
- **Mock-Anbieter:** das erste Token nach 20 ms, dann 40 Tokens im Abstand von 5 ms. Im Kapazitätslauf antwortet er sofort.
- **Last:** 1, 4, 16 und 64 Clients mit 1.000, 2.000, 4.000 und 16.000 Anfragen pro Runde.
- **Runden:** drei, die Gateways in jeder Runde neu zufällig geordnet. Jedes Gateway wurde direkt nach einem direkten Lauf derselben Form gemessen.
- **CPU und Speicher:** jeder Prozess im Container des Gateways, viermal pro Sekunde gelesen, und ebenso seine Datenbanken.

Die Images liefen mit diesen Digests:

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.0@sha256:3d32fd7827417e49475b4ac9ad7d25f7daed28f02fa489ef4077ff501faaae1a
ghcr.io/berriai/litellm:v1.110.2@sha256:ef798594ca3db8d81bad2e1439f536f49b71fe095ee9f20416f5e149b4e22035
maximhq/bifrost:v2.6.0@sha256:5968d007ba4f74ee01bc4ceb071decb9927a0b7bb43dbb9072208f75e2d522e3
portkeyai/gateway:1.15.2@sha256:97f094d9c8a764cbfaa2a7138c0017b247ca923bb06db1b4c13b7f8a33b5200d
```

LiteLLM Rust hat noch kein veröffentlichtes Image. Es wurde aus `BerriAI/litellm` bei `d4e8f96` mit `rust:1.98.0` gebaut.

> [!IMPORTANT]
> Der Autor von Kaeso hat diesen Benchmark auf einer Maschine gemessen. BerriAI nennt seine eigenen veröffentlichten Ergebnisse Single-Host und vom Anbieter gemessen; das gilt hier genauso. Messen Sie selbst nach, bevor Sie sich darauf verlassen.

## Selbst messen

Nehmen Sie eine frische Ubuntu-24.04-Maschine mit vier Kernen und arbeiten Sie als root. Das Setup installiert Docker, baut den Benchmark und LiteLLM Rust bei denselben Commits und lädt jedes Image über seinen Digest:

```bash
curl -fsSLO https://kaeso.app/benchmarks/2026-11-22/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
```

Drei Runden aller Gateways dauern etwa 90 Minuten, der Kapazitätslauf weitere 30. Die Ergebnisse landen in `/root/out` und `/root/cap`.

## Nächste Schritte

- [Anforderungen](requirements.md): Bemessen Sie den Server für den Verkehr, den Sie erwarten.
- [Mehrere Gateways](scaling.md): Betreiben Sie weitere Gateways, wenn eines nicht reicht.
- [Gateway-Benchmark](../development/benchmark.md): Messen Sie Kaeso allein mit Streams, Abbrüchen und langsamen Apps.
