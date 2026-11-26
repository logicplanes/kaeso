---
title: Leistung
description: Sehen Sie, wie viel Zeit, CPU und Speicher Kaeso neben LiteLLM, Bifrost und Portkey in BerriAIs Benchmark hinzufügt.
order: 14
---

Kaeso 0.7.4 lief in [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench), dem Benchmark von BerriAI, der Firma hinter LiteLLM. LiteLLM, Bifrost und Portkey liefen daneben auf derselben Maschine, ebenso jede frühere 0.7-Version von Kaeso. Nutzen Sie die Zahlen, um einen Server zu bemessen und Kaeso mit dem Gateway zu vergleichen, das Sie heute betreiben. [kaeso.app/benchmarks](https://kaeso.app/de/benchmarks) zeigt sie als Diagramme und behält jeden Lauf, jeweils mit seinen Skripten und Rohdaten.

Jede Zahl ist der Median aus drei Runden. Der Mock-Anbieter des Benchmarks steht für den Modellanbieter, daher zeigen die Zahlen die Kosten des Gateways selbst, nicht die eines Modells.

## Kapazität

Diese Zahlen stammen von 64 Clients gegen einen Mock, der sofort antwortet, jedes Gateway auf denselben zwei dedizierten Kernen. Derselbe Treiber schaffte direkt zum Mock etwa 23.200 Anfragen pro Sekunde, also lief jedes Gateway an seiner eigenen Grenze:

| Gateway | Anfragen/s | Latenz p50 / p99 | CPU pro Anfrage | Höchster Speicher |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | 4.551 | 13,8 / 21,6 ms | 437 µs | 163 + 594 MB |
| Kaeso 0.7.4, Anfrageprotokolle an | 3.962 | 15,8 / 27,2 ms | 500 µs | 188 + 1.489 MB |
| LiteLLM Rust, `main` bei `d4e8f96` | 2.955 | 21,4 / 35,5 ms | 642 µs | 87 MB |
| Bifrost 2.6.0 | 2.242 | 24,7 / 63,9 ms | 887 µs | 414 MB |
| Portkey 1.15.2 | 619 | 101,2 / 148,6 ms | 1.829 µs | 303 MB |
| LiteLLM Python 1.110.2 | 55 | 1.162,0 / 1.331,9 ms | 18.286 µs | 742 MB |

Die CPU pro Anfrage ist die des Gateways. Kaesos Speicher ist der des Gateways plus der seiner Datenbanken: PostgreSQL und Valkey, mit Anfrageprotokollen auch ClickHouse. Die anderen Gateways liefen ohne Datenbank. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) zeigt die Spanne jeder Runde.

## Zusätzliche Latenz

Zusätzliche Latenz ist die Zeit einer Anfrage durch das Gateway minus die Zeit derselben Anfrage direkt zum Mock. Der Mock antwortet wie ein schneller Anbieter in etwa 22 ms. Jede Zelle zeigt p50 / p99:

| Gateway | 1 Client | 4 Clients | 16 Clients |
| --- | --- | --- | --- |
| Kaeso 0.7.4 | +0,94 / +1,96 ms | +0,92 / +1,81 ms | +0,64 / +1,86 ms |
| Kaeso 0.7.4, Anfrageprotokolle an | +0,91 / +2,24 ms | +0,99 / +2,53 ms | +0,77 / +2,08 ms |
| LiteLLM Rust, `main` bei `d4e8f96` | +1,05 / +1,16 ms | +0,97 / +1,67 ms | +0,82 / +2,43 ms |
| Bifrost 2.6.0 | +1,39 / +1,75 ms | +1,31 / +2,76 ms | +0,89 / +5,03 ms |
| Portkey 1.15.2 | +2,63 / +6,76 ms | +2,35 / +9,35 ms | +5,1 / +24,4 ms |
| LiteLLM Python 1.110.2 | +20,4 / +22,2 ms | +65,2 / +76,7 ms | +260 / +284 ms |

Eine Zeile für 64 Clients fehlt. Bei dieser Last teilen sich Mock und Treiber zwei Kerne und kommen nicht über etwa 2.900 Anfragen pro Sekunde. Den direkten Lauf abzuziehen, würde dann die Zeit jedes Gateways zu klein ausweisen; diese Last misst stattdessen die Kapazitätstabelle.

## Streams und Agent-Sitzungen

Der Streaming-Lauf sendet 200 gestreamte Anfragen bei 16 Clients. Die Sitzungsläufe spielen eine aufgezeichnete Claude-Code-Sitzung und eine aufgezeichnete Codex-Sitzung ab, je 30 Turns. Die zusätzliche Zeit einer Sitzung ist ihre Gesamtzeit durch das Gateway minus direkt zum Mock:

| Gateway | Zusätzliche Zeit bis zum ersten Token | Chunk-Abstand p99 | Claude-Code-Sitzung | Codex-Sitzung |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | +1,08 ms | 8,0 ms | +50,7 ms | +52,1 ms |
| Kaeso 0.7.4, Anfrageprotokolle an | +1,11 ms | 9,0 ms | +60,2 ms | +80,7 ms |
| LiteLLM Rust, `main` bei `d4e8f96` | +1,32 ms | 40,3 ms | +54,3 ms | +59,3 ms |
| Bifrost 2.6.0 | Nicht vergleichbar | 20,4 ms | +168,8 ms | +104,8 ms |
| Portkey 1.15.2 | +28,20 ms | 9,8 ms | +121,0 ms | +125,2 ms |
| LiteLLM Python 1.110.2 | +227,63 ms | 50,5 ms | +642,6 ms | +647,0 ms |

Direkt zum Mock liegt der Chunk-Abstand bei 7,7 ms im p99. Bifrost sendet sein erstes Ereignis, bevor der Anbieter antwortet, und der Benchmark zählt das als erstes Token. Deshalb ist seine Zeit bis zum ersten Token negativ. Jede gestreamte Sitzung lieferte durch jedes Gateway gültige Tool-Aufrufe.

## Wo andere Gateways vorn liegen

- **Speicher:** LiteLLM Rust kam nie über 87 MB und braucht keine Datenbank. Kaeso erreichte 163 MB, und seine PostgreSQL und Valkey belegten weitere 594 MB.
- **Ein Client:** Mit einer Anfrage zur Zeit lag das p99 von LiteLLM Rust bei +1,16 ms, das von Kaeso bei +1,96 ms, und keine Runde von Kaeso kam näher.
- **Anfrageprotokolle:** Mit jeder Anfrage in ClickHouse schaffte Kaeso 13 % weniger Anfragen pro Sekunde, und seine Datenbanken belegten 1.489 MB.

## Jede 0.7-Version auf einer Maschine

Kaeso 0.7.0, 0.7.1, 0.7.2, 0.7.3 und 0.7.4 liefen in diesem Lauf jeweils als eigenes Gateway, in denselben gemischten Runden wie die anderen Gateways. So vergleichen sich die Versionen Zahl für Zahl, ohne die Maschine oder die Stunde dazwischen. Ein Schritt zählt nur, wo sich die Runden zweier Versionen nicht überschneiden, und ein einmal gemessener Wert wie CPU oder höchster Speicher nur, wo er sich um mehr als ein Zehntel bewegte:

| Version | Anfragen/s | Mit Anfrageprotokollen | Zusätzlich p50, 1 / 4 / 16 Clients | Zusätzlich p99, 16 Clients | CPU pro Anfrage | Höchster Speicher |
| --- | --- | --- | --- | --- | --- | --- |
| Kaeso 0.7.4 | 4.551 | 3.962 | +0,94 / +0,92 / +0,64 ms | +1,86 ms | 437 µs | 163 + 594 MB |
| Kaeso 0.7.3 | 5.204 | 4.371 | +1,02 / +1,06 / +0,73 ms | +1,67 ms | 384 µs | 102 + 543 MB |
| Kaeso 0.7.2 | 4.666 | 4.032 | +0,92 / +0,90 / +0,66 ms | +1,70 ms | 428 µs | 178 + 579 MB |
| Kaeso 0.7.1 | 4.413 | 3.781 | +0,96 / +0,99 / +0,83 ms | +1,90 ms | 453 µs | 110 + 565 MB |
| Kaeso 0.7.0 | 4.748 | 3.891 | +0,98 / +0,99 / +0,84 ms | +2,03 ms | 421 µs | 113 + 283 MB |

- **0.7.3 zu 0.7.4:** 0.7.4 nimmt das Warten zurück, das 0.7.3 jeder Antwort hinzugefügt hatte, und der Median, den es bei 1 und 4 Clients hinzufügt, liegt wieder unter dem von 0.7.3. 0.7.3 schaffte in jeder Runde die meisten Anfragen aller Versionen; 0.7.4 schaffte 13 % weniger, mit 14 % mehr CPU pro Anfrage, und liegt damit innerhalb der Runden von 0.7.2. Sein höchster Speicher unter Volllast stieg von 102 auf 163 MB.
- **0.7.2 zu 0.7.3:** mehr Anfragen pro Sekunde und weniger CPU pro Anfrage, ein höherer Median bei 1, 4 und 16 Clients und weniger höchster Speicher, 102 statt 178 MB.
- **0.7.1 zu 0.7.2:** mehr Anfragen pro Sekunde, mit und ohne Anfrageprotokolle, ein niedrigerer Median bei 1 und 4 Clients und mehr höchster Speicher, 178 statt 110 MB.
- **0.7.0 zu 0.7.1:** 7 % weniger Anfragen pro Sekunde, und der höchste Speicher der Datenbanken verdoppelte sich; dort blieb er in jeder späteren Version.

Die Runden zeigen jede Änderung, nicht ihre Ursache. Die früheren Läufe, [0.7.3](https://kaeso.app/de/benchmarks/0.7.3), [0.7.2](https://kaeso.app/de/benchmarks/0.7.2) und [0.7.0](https://kaeso.app/de/benchmarks/0.7.0), liefen auf anderen Maschinen desselben Typs und bleiben mit ihren Skripten und Rohdaten veröffentlicht.
## Was jedes Gateway tat

Jedes Gateway lief so, wie sein Projekt es ausliefert, gegen den Mock im Format Anthropic Messages:

| Gateway | Bei jeder Anfrage geprüft | Gespeichert |
| --- | --- | --- |
| Kaeso 0.7.4 | Den [API-Schlüssel](../console/api-keys.md) eines Projekts und die Modelle, die das Projekt erlaubt | Nutzung in PostgreSQL; keine Anfrageprotokolle |
| Kaeso 0.7.4, Anfrageprotokolle an | Dasselbe | Nutzung in PostgreSQL und jede Anfrage in ClickHouse |
| LiteLLM Rust, `main` bei `d4e8f96`, unveröffentlicht | Einen Master-Schlüssel aus seiner Konfiguration | Nichts; keine Datenbank |
| LiteLLM Python 1.110.2 | Einen Master-Schlüssel aus seiner Konfiguration | Nichts; keine Datenbank |
| Bifrost 2.6.0 | Keinen Schlüssel | Nichts; Logging und Stores aus |
| Portkey 1.15.2 | Keinen Schlüssel | Nichts; Logging aus |

## Wie es lief

- **Maschine:** eine DigitalOcean CPU-Optimized c-4 mit 4 dedizierten vCPUs (Intel Xeon Platinum 8280, 2,7 GHz), 8 GB Speicher, Ubuntu 24.04 und Docker, am 25. und 26. November 2026. Vor dem Lauf wurden ihre geplanten Jobs, Paket-Updates, snapd und die Agents des Anbieters abgeschaltet, ebenso Transparent Huge Pages und das Auslagern. Derselbe Treiber schaffte dort direkt zum Mock etwa 23.200 Anfragen pro Sekunde, wo die Maschinen der Läufe von 0.7.0 und 0.7.2 etwa 28.000 schafften; deshalb liegen die Zahlen jedes Gateways niedriger als in diesen Läufen.
- **Kerne:** Jedes Gateway lief in Docker im Host-Netzwerk, fest auf zwei Kernen. Mock, Lasttreiber und Kaesos Datenbanken liefen auf den anderen beiden.
- **Benchmark:** `BerriAI/ai-gateway-bench` bei `44365ca`, Mock-Anbieter und Treiber unverändert: `load-probe` für Anfragen, `run_http_bench.py` für Streams und `session_runner.py` für Sitzungen.
- **Mock-Anbieter:** das erste Token nach 20 ms, dann 40 Tokens im Abstand von 5 ms. Im Kapazitätslauf antwortet er sofort.
- **Last:** 1, 4, 16 und 64 Clients mit 1.000, 2.000, 4.000 und 16.000 Anfragen pro Runde.
- **Runden:** drei, die Gateways in jeder Runde neu zufällig geordnet. Jedes Gateway wurde direkt nach einem direkten Lauf derselben Form gemessen.
- **CPU und Speicher:** jeder Prozess im Container des Gateways, viermal pro Sekunde gelesen, und ebenso seine Datenbanken.

Die Images liefen mit diesen Digests:

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.4@sha256:25fa94e10da01773b0823398ba72c2e1faec3521f73ea9725db877b615be83b3
ghcr.io/logicplanes/kaeso-gateway:0.7.3@sha256:e070906766362fc298babb51d2d0ee9a60deb07cbf30aac2e408f6d193f91553
ghcr.io/logicplanes/kaeso-gateway:0.7.2@sha256:7db4ee52773bc546746fb5e3d513457929d1782fdb89795f7b9c9f406a94ae79
ghcr.io/logicplanes/kaeso-gateway:0.7.1@sha256:42f7e818b43e9422a432747a9841df8952819de4c8e557f5dae8216f76409e7e
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
curl -fsSLO https://kaeso.app/benchmarks/runs/0.7.4/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
```

Drei Runden aller Gateways dauern etwa zwei Stunden, der Kapazitätslauf weitere 20 Minuten. Die Ergebnisse landen in `/root/out` und `/root/cap`. Jede Kaeso-Version ist ein eigenes Gateway, `kaeso-0.7.4`, `kaeso-logs-0.7.4` und so weiter; nennen Sie Gateways nach `run.py`, um nur diese zu messen. Absolute Zahlen ändern sich von Maschine zu Maschine; vergleichen Sie Gateways aus demselben Lauf.

## Nächste Schritte

- [Anforderungen](requirements.md): Bemessen Sie den Server für den Verkehr, den Sie erwarten.
- [Mehrere Gateways](scaling.md): Betreiben Sie weitere Gateways, wenn eines nicht reicht.
- [Gateway-Benchmark](../development/benchmark.md): Messen Sie Kaeso allein mit Streams, Abbrüchen und langsamen Apps.
