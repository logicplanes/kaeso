---
title: Leistung
description: Sehen Sie, wie viel Zeit, CPU und Speicher Kaeso neben LiteLLM, Bifrost und Portkey in BerriAIs Benchmark hinzufügt.
order: 14
---

Kaeso 0.7.2 lief in [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench), dem Benchmark von BerriAI, der Firma hinter LiteLLM. LiteLLM, Bifrost und Portkey liefen daneben auf derselben Maschine. Nutzen Sie die Zahlen, um einen Server zu bemessen und Kaeso mit dem Gateway zu vergleichen, das Sie heute betreiben. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) zeigt sie als Diagramme, mit den Skripten und allen Rohdaten.

Jede Zahl ist der Median aus drei Runden. Der Mock-Anbieter des Benchmarks steht für den Modellanbieter, daher zeigen die Zahlen die Kosten des Gateways selbst, nicht die eines Modells.

## Kapazität

Diese Zahlen stammen von 64 Clients gegen einen Mock, der sofort antwortet, jedes Gateway auf denselben zwei dedizierten Kernen. Derselbe Treiber schaffte direkt zum Mock etwa 28.000 Anfragen pro Sekunde, also lief jedes Gateway an seiner eigenen Grenze:

| Gateway | Anfragen/s | Latenz p50 / p99 | CPU pro Anfrage | Höchster Speicher |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.2 | 6.243 | 10,2 / 15,9 ms | 320 µs | 104 + 559 MB |
| Kaeso 0.7.2, Anfrageprotokolle an | 5.177 | 12,3 / 20,2 ms | 385 µs | 212 + 1.453 MB |
| LiteLLM Rust, `main` bei `d4e8f96` | 3.286 | 19,3 / 31,9 ms | 560 µs | 86 MB |
| Bifrost 2.6.0 | 2.858 | 20,6 / 52,3 ms | 693 µs | 452 MB |
| Portkey 1.15.2 | 821 | 77,9 / 113,5 ms | 1.366 µs | 301 MB |
| LiteLLM Python 1.110.2 | 75 | 849,0 / 918,0 ms | 13.381 µs | 744 MB |

Die CPU pro Anfrage ist die des Gateways. Kaesos Speicher ist der des Gateways plus der seiner Datenbanken: PostgreSQL und Valkey, mit Anfrageprotokollen auch ClickHouse. Die anderen Gateways liefen ohne Datenbank. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) zeigt die Spanne jeder Runde.

## Zusätzliche Latenz

Zusätzliche Latenz ist die Zeit einer Anfrage durch das Gateway minus die Zeit derselben Anfrage direkt zum Mock. Der Mock antwortet wie ein schneller Anbieter in etwa 22 ms. Jede Zelle zeigt p50 / p99:

| Gateway | 1 Client | 4 Clients | 16 Clients |
| --- | --- | --- | --- |
| Kaeso 0.7.2 | +0,87 / +1,63 ms | +0,66 / +1,23 ms | +0,79 / +1,73 ms |
| Kaeso 0.7.2, Anfrageprotokolle an | +0,67 / +1,62 ms | +0,52 / +1,97 ms | +0,56 / +1,83 ms |
| LiteLLM Rust, `main` bei `d4e8f96` | +1,01 / +1,09 ms | +0,89 / +2,11 ms | +0,67 / +2,33 ms |
| Bifrost 2.6.0 | +1,09 / +1,24 ms | +0,96 / +2,36 ms | +0,57 / +2,98 ms |
| Portkey 1.15.2 | +2,12 / +4,79 ms | +1,11 / +6,49 ms | +2,9 / +16,4 ms |
| LiteLLM Python 1.110.2 | +15,8 / +17,3 ms | +49,3 / +57,8 ms | +185 / +206 ms |

Eine Zeile für 64 Clients fehlt. Bei dieser Last teilen sich Mock und Treiber zwei Kerne und kommen nicht über etwa 2.900 Anfragen pro Sekunde. Den direkten Lauf abzuziehen, würde dann die Zeit jedes Gateways zu klein ausweisen; diese Last misst stattdessen die Kapazitätstabelle.

## Streams und Agent-Sitzungen

Der Streaming-Lauf sendet 200 gestreamte Anfragen bei 16 Clients. Die Sitzungsläufe spielen eine aufgezeichnete Claude-Code-Sitzung und eine aufgezeichnete Codex-Sitzung ab, je 30 Turns. Die zusätzliche Zeit einer Sitzung ist ihre Gesamtzeit durch das Gateway minus direkt zum Mock:

| Gateway | Zusätzliche Zeit bis zum ersten Token | Chunk-Abstand p99 | Claude-Code-Sitzung | Codex-Sitzung |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.2 | +0,95 ms | 7,7 ms | +43,5 ms | +44,8 ms |
| Kaeso 0.7.2, Anfrageprotokolle an | +0,98 ms | 8,0 ms | +42,1 ms | +58,6 ms |
| LiteLLM Rust, `main` bei `d4e8f96` | +0,94 ms | 40,4 ms | +45,2 ms | +46,6 ms |
| Bifrost 2.6.0 | Nicht vergleichbar | 20,6 ms | +125,3 ms | +94,5 ms |
| Portkey 1.15.2 | +27,54 ms | 8,1 ms | +100,0 ms | +104,5 ms |
| LiteLLM Python 1.110.2 | +114,17 ms | 33,3 ms | +510,5 ms | +504,6 ms |

Direkt zum Mock liegt der Chunk-Abstand bei 7,5 ms im p99. Bifrost sendet sein erstes Ereignis, bevor der Anbieter antwortet, und der Benchmark zählt das als erstes Token. Deshalb ist seine Zeit bis zum ersten Token negativ. Jede gestreamte Sitzung lieferte durch jedes Gateway gültige Tool-Aufrufe.

## Wo andere Gateways vorn liegen

- **Speicher:** LiteLLM Rust kam nie über 86 MB und braucht keine Datenbank. Kaeso erreichte 160 MB, und seine PostgreSQL und Valkey belegten weitere 559 MB.
- **Sechzehn Clients:** Im Median fügte Bifrost +0,57 ms hinzu und LiteLLM Rust +0,67 ms, Kaeso +0,79 ms, und jede Runde von Bifrost lag unter jeder Runde von Kaeso. Beim p99 fügte Kaeso am wenigsten hinzu.
- **Gleichstand:** Die Zeit bis zum ersten Token und die Claude-Code-Sitzung liegen in der Spanne des jeweils anderen.
- **Anfrageprotokolle:** Mit jeder Anfrage in ClickHouse schaffte Kaeso 17 % weniger Anfragen pro Sekunde, und seine Datenbanken belegten 1.453 MB.

## Seit Kaeso 0.7.0

Der Lauf vom 22. November 2026 maß Kaeso 0.7.0 mit demselben Benchmark, denselben Skripten und Images aller anderen Gateways, auf einer anderen c-4 desselben Typs. Die Kapazität der unveränderten Gateways verschob sich zwischen den beiden Läufen um bis zu 5 %. Ein Unterschied zählt deshalb nur, wo sich die Runden beider Läufe nicht überschneiden und die anderen Gateways sich nicht genauso bewegten:

| Messgröße | 0.7.0 | 0.7.2 | Einordnung |
| --- | --- | --- | --- |
| Anfragen pro Sekunde unter Volllast | 6.338 | 6.243 | Gleich: die Runden überschneiden sich |
| Dasselbe mit Anfrageprotokollen | 5.324 | 5.177 | Gleich: die Runden überschneiden sich |
| Zusätzlich im Median, 1 Client | +0,73 ms | +0,87 ms | Höher, aber LiteLLM Rust stieg etwa gleich stark: die Maschine |
| Zusätzlich im Median, 4 Clients | +0,56 ms | +0,66 ms | Höher, aber LiteLLM Rust und Bifrost stiegen etwa gleich stark: die Maschine |
| Zusätzlich im Median, 16 Clients | +0,54 ms | +0,79 ms | Höher, außerhalb der Runden beider Läufe |
| Zusätzlich beim p99, 16 Clients | +1,47 ms | +1,73 ms | Gleich: die Runden überschneiden sich |
| Chunk-Abstand p99 | 7,8 ms | 7,7 ms | Gleich: die Runden überschneiden sich |
| Höchster Speicher unter Volllast, mit Anfrageprotokollen | 450 MB | 212 MB | Niedriger |
| Höchster Speicher von Kaesos Datenbanken unter Volllast | 271 MB | 559 MB | Höher |

0.7.2 hält die Zeilen des Anfrageprotokolls, die auf ClickHouse warten, innerhalb von [`logs.buffer_size`](configuration.md), was zum niedrigeren Speicher mit Anfrageprotokollen passt. Kaesos PostgreSQL belegte in der Spitze etwa doppelt so viel Speicher, während seine CPU pro Anfrage gleich blieb, und Kaesos zusätzlicher Median bei 16 Clients stieg, während der der anderen Gateways nicht stieg. Die Läufe zeigen nicht, ob 0.7.2 oder die Maschine diese beiden verursachte. Die [Skripte](https://kaeso.app/benchmarks/2026-11-22/harness.tar.gz) und [Rohdaten](https://kaeso.app/benchmarks/2026-11-22/raw.tar.gz) des 0.7.0-Laufs bleiben veröffentlicht.

## Was jedes Gateway tat

Jedes Gateway lief so, wie sein Projekt es ausliefert, gegen den Mock im Format Anthropic Messages:

| Gateway | Bei jeder Anfrage geprüft | Gespeichert |
| --- | --- | --- |
| Kaeso 0.7.2 | Den [API-Schlüssel](../console/api-keys.md) eines Projekts und die Modelle, die das Projekt erlaubt | Nutzung in PostgreSQL; keine Anfrageprotokolle |
| Kaeso 0.7.2, Anfrageprotokolle an | Dasselbe | Nutzung in PostgreSQL und jede Anfrage in ClickHouse |
| LiteLLM Rust, `main` bei `d4e8f96`, unveröffentlicht | Einen Master-Schlüssel aus seiner Konfiguration | Nichts; keine Datenbank |
| LiteLLM Python 1.110.2 | Einen Master-Schlüssel aus seiner Konfiguration | Nichts; keine Datenbank |
| Bifrost 2.6.0 | Keinen Schlüssel | Nichts; Logging und Stores aus |
| Portkey 1.15.2 | Keinen Schlüssel | Nichts; Logging aus |

## Wie es lief

- **Maschine:** eine DigitalOcean CPU-Optimized c-4 mit 4 dedizierten vCPUs (Intel Xeon Platinum 8358, 2,6 GHz), 8 GB Speicher, Ubuntu 24.04 und Docker, am 25. November 2026.
- **Kerne:** Jedes Gateway lief in Docker im Host-Netzwerk, fest auf zwei Kernen. Mock, Lasttreiber und Kaesos Datenbanken liefen auf den anderen beiden.
- **Benchmark:** `BerriAI/ai-gateway-bench` bei `44365ca`, Mock-Anbieter und Treiber unverändert: `load-probe` für Anfragen, `run_http_bench.py` für Streams und `session_runner.py` für Sitzungen.
- **Mock-Anbieter:** das erste Token nach 20 ms, dann 40 Tokens im Abstand von 5 ms. Im Kapazitätslauf antwortet er sofort.
- **Last:** 1, 4, 16 und 64 Clients mit 1.000, 2.000, 4.000 und 16.000 Anfragen pro Runde.
- **Runden:** drei, die Gateways in jeder Runde neu zufällig geordnet. Jedes Gateway wurde direkt nach einem direkten Lauf derselben Form gemessen.
- **CPU und Speicher:** jeder Prozess im Container des Gateways, viermal pro Sekunde gelesen, und ebenso seine Datenbanken.

Die Images liefen mit diesen Digests:

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.2@sha256:7db4ee52773bc546746fb5e3d513457929d1782fdb89795f7b9c9f406a94ae79
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
curl -fsSLO https://kaeso.app/benchmarks/2026-11-25/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
```

Drei Runden aller Gateways dauern etwa 90 Minuten, der Kapazitätslauf weitere 30. Die Ergebnisse landen in `/root/out` und `/root/cap`. Absolute Zahlen ändern sich von Maschine zu Maschine; vergleichen Sie Gateways aus demselben Lauf.

## Nächste Schritte

- [Anforderungen](requirements.md): Bemessen Sie den Server für den Verkehr, den Sie erwarten.
- [Mehrere Gateways](scaling.md): Betreiben Sie weitere Gateways, wenn eines nicht reicht.
- [Gateway-Benchmark](../development/benchmark.md): Messen Sie Kaeso allein mit Streams, Abbrüchen und langsamen Apps.
