---
title: Leistung
description: Sehen Sie, wie viel Zeit, CPU und Speicher Kaeso neben LiteLLM, Bifrost und Portkey in BerriAIs Benchmark hinzufügt.
order: 14
---

Kaeso 0.7.3 lief in [ai-gateway-bench](https://github.com/BerriAI/ai-gateway-bench), dem Benchmark von BerriAI, der Firma hinter LiteLLM. LiteLLM, Bifrost und Portkey liefen daneben auf derselben Maschine. Nutzen Sie die Zahlen, um einen Server zu bemessen und Kaeso mit dem Gateway zu vergleichen, das Sie heute betreiben. [kaeso.app/benchmarks](https://kaeso.app/de/benchmarks) zeigt sie als Diagramme und behält jeden Lauf, jeweils mit seinen Skripten und Rohdaten.

Jede Zahl ist der Median aus drei Runden. Der Mock-Anbieter des Benchmarks steht für den Modellanbieter, daher zeigen die Zahlen die Kosten des Gateways selbst, nicht die eines Modells.

## Kapazität

Diese Zahlen stammen von 64 Clients gegen einen Mock, der sofort antwortet, jedes Gateway auf denselben zwei dedizierten Kernen. Derselbe Treiber schaffte direkt zum Mock etwa 19.600 Anfragen pro Sekunde, also lief jedes Gateway an seiner eigenen Grenze:

| Gateway | Anfragen/s | Latenz p50 / p99 | CPU pro Anfrage | Höchster Speicher |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.3 | 4.835 | 12,8 / 22,6 ms | 412 µs | 181 + 633 MB |
| Kaeso 0.7.3, Anfrageprotokolle an | 3.914 | 15,9 / 28,3 ms | 506 µs | 179 + 1.476 MB |
| LiteLLM Rust, `main` bei `d4e8f96` | 2.068 | 30,4 / 53,7 ms | 842 µs | 86 MB |
| Bifrost 2.6.0 | 2.088 | 27,9 / 68,7 ms | 954 µs | 422 MB |
| Portkey 1.15.2 | 599 | 104,1 / 159,8 ms | 1.908 µs | 290 MB |
| LiteLLM Python 1.110.2 | 39 | 1.622,0 / 1.827,0 ms | 25.484 µs | 742 MB |

Die CPU pro Anfrage ist die des Gateways. Kaesos Speicher ist der des Gateways plus der seiner Datenbanken: PostgreSQL und Valkey, mit Anfrageprotokollen auch ClickHouse. Die anderen Gateways liefen ohne Datenbank. [kaeso.app/benchmarks](https://kaeso.app/benchmarks) zeigt die Spanne jeder Runde.

## Zusätzliche Latenz

Zusätzliche Latenz ist die Zeit einer Anfrage durch das Gateway minus die Zeit derselben Anfrage direkt zum Mock. Der Mock antwortet wie ein schneller Anbieter in etwa 22 ms. Jede Zelle zeigt p50 / p99:

| Gateway | 1 Client | 4 Clients | 16 Clients |
| --- | --- | --- | --- |
| Kaeso 0.7.3 | +0,81 / +1,97 ms | +0,67 / +2,16 ms | +0,53 / +1,81 ms |
| Kaeso 0.7.3, Anfrageprotokolle an | +0,83 / +2,44 ms | +0,68 / +2,39 ms | +0,55 / +1,93 ms |
| LiteLLM Rust, `main` bei `d4e8f96` | +1,03 / +1,32 ms | +0,78 / +1,95 ms | +0,88 / +4,83 ms |
| Bifrost 2.6.0 | +1,28 / +1,79 ms | +1,02 / +2,60 ms | +1,01 / +4,10 ms |
| Portkey 1.15.2 | +2,32 / +6,63 ms | +1,44 / +9,36 ms | +5,9 / +30,1 ms |
| LiteLLM Python 1.110.2 | +26,5 / +29,7 ms | +87 / +110 ms | +376 / +420 ms |

Eine Zeile für 64 Clients fehlt. Bei dieser Last teilen sich Mock und Treiber zwei Kerne und kommen nicht über etwa 2.900 Anfragen pro Sekunde. Den direkten Lauf abzuziehen, würde dann die Zeit jedes Gateways zu klein ausweisen; diese Last misst stattdessen die Kapazitätstabelle.

## Streams und Agent-Sitzungen

Der Streaming-Lauf sendet 200 gestreamte Anfragen bei 16 Clients. Die Sitzungsläufe spielen eine aufgezeichnete Claude-Code-Sitzung und eine aufgezeichnete Codex-Sitzung ab, je 30 Turns. Die zusätzliche Zeit einer Sitzung ist ihre Gesamtzeit durch das Gateway minus direkt zum Mock:

| Gateway | Zusätzliche Zeit bis zum ersten Token | Chunk-Abstand p99 | Claude-Code-Sitzung | Codex-Sitzung |
| --- | --- | --- | --- | --- |
| Kaeso 0.7.3 | +1,21 ms | 8,7 ms | +63,4 ms | +65,6 ms |
| Kaeso 0.7.3, Anfrageprotokolle an | +1,49 ms | 9,5 ms | +71,1 ms | +68,8 ms |
| LiteLLM Rust, `main` bei `d4e8f96` | +1,27 ms | 40,1 ms | +64,4 ms | +54,4 ms |
| Bifrost 2.6.0 | Nicht vergleichbar | 20,2 ms | +183,9 ms | +118,8 ms |
| Portkey 1.15.2 | +28,19 ms | 9,3 ms | +114,2 ms | +120,5 ms |
| LiteLLM Python 1.110.2 | +402,27 ms | 71,2 ms | +814,8 ms | +835,7 ms |

Direkt zum Mock liegt der Chunk-Abstand bei 8,3 ms im p99. Bifrost sendet sein erstes Ereignis, bevor der Anbieter antwortet, und der Benchmark zählt das als erstes Token. Deshalb ist seine Zeit bis zum ersten Token negativ. Jede gestreamte Sitzung lieferte durch jedes Gateway gültige Tool-Aufrufe.

## Wo andere Gateways vorn liegen

- **Speicher:** LiteLLM Rust kam nie über 87 MB und braucht keine Datenbank. Kaeso erreichte 181 MB, und seine PostgreSQL und Valkey belegten weitere 633 MB.
- **Ein Client:** Mit einer Anfrage zur Zeit lag das p99 von LiteLLM Rust bei +1,32 ms, das von Kaeso bei +1,97 ms, und keine Runde von Kaeso kam näher.
- **Gleichstand:** Die Zeit bis zum ersten Token und die Claude-Code-Sitzung liegen in der Spanne des jeweils anderen.
- **Anfrageprotokolle:** Mit jeder Anfrage in ClickHouse schaffte Kaeso 19 % weniger Anfragen pro Sekunde, und seine Datenbanken belegten 1.476 MB.

## Seit Kaeso 0.7.2

Kaeso 0.7.2 lief direkt nach 0.7.3 noch einmal auf der Maschine dieses Laufs, mit und ohne Anfrageprotokolle, mit denselben Skripten. So vergleichen sich die beiden Versionen ohne die Maschine dazwischen. Ein Unterschied zählt nur, wo sich die Runden beider nicht überschneiden, und ein einmal gemessener Wert wie CPU oder höchster Speicher nur, wo er sich um mehr als ein Zehntel bewegte:

| Messgröße | 0.7.2 | 0.7.3 | Einordnung |
| --- | --- | --- | --- |
| Anfragen pro Sekunde unter Volllast | 4.678 | 4.835 | Höher, außerhalb der Runden beider |
| Dasselbe mit Anfrageprotokollen | 3.849 | 3.914 | Gleich: die Runden überschneiden sich |
| CPU pro Anfrage unter Volllast | 427 µs | 412 µs | Gleich: innerhalb eines Zehntels |
| Zusätzlich im Median, 1 Client | +0,76 ms | +0,81 ms | Höher, außerhalb der Runden beider |
| Zusätzlich im Median, 4 Clients | +0,55 ms | +0,67 ms | Höher, außerhalb der Runden beider |
| Zusätzlich im Median, 16 Clients | +0,56 ms | +0,53 ms | Gleich: die Runden überschneiden sich |
| Zusätzlich beim p99, 16 Clients | +1,54 ms | +1,81 ms | Gleich: die Runden überschneiden sich |
| Chunk-Abstand p99 | 8,7 ms | 8,7 ms | Gleich: die Runden überschneiden sich |
| Höchster Speicher unter Volllast, mit Anfrageprotokollen | 168 MB | 179 MB | Gleich: innerhalb eines Zehntels |
| Höchster Speicher von Kaesos Datenbanken unter Volllast | 567 MB | 633 MB | Höher, um mehr als ein Zehntel |

Die beiden Versionen liefen nacheinander, nicht abwechselnd. Eine Änderung von 0.7.3 liegt auf dem Weg dieses Benchmarks: Bevor eine Antwort hinausgeht, vermerkt das Gateway jetzt die Anfrage, damit die Kopie einer [Quelle](../concepts/sources.md) nur einmal zählt; für eine Organisation ohne Quellen ist das eine zwischengespeicherte Prüfung in einem eigenen Task. Ob das oder die Maschine in der Stunde zwischen beiden Unterschiede von 3 % bei der Kapazität und 0,05 bis 0,12 ms im Median erklärt, zeigen die Runden nicht. Die früheren Läufe, [0.7.2](https://kaeso.app/de/benchmarks/0.7.2) und [0.7.0](https://kaeso.app/de/benchmarks/0.7.0), liefen auf anderen Maschinen desselben Typs und bleiben mit ihren Skripten und Rohdaten veröffentlicht.

## Was jedes Gateway tat

Jedes Gateway lief so, wie sein Projekt es ausliefert, gegen den Mock im Format Anthropic Messages:

| Gateway | Bei jeder Anfrage geprüft | Gespeichert |
| --- | --- | --- |
| Kaeso 0.7.3 | Den [API-Schlüssel](../console/api-keys.md) eines Projekts und die Modelle, die das Projekt erlaubt | Nutzung in PostgreSQL; keine Anfrageprotokolle |
| Kaeso 0.7.3, Anfrageprotokolle an | Dasselbe | Nutzung in PostgreSQL und jede Anfrage in ClickHouse |
| LiteLLM Rust, `main` bei `d4e8f96`, unveröffentlicht | Einen Master-Schlüssel aus seiner Konfiguration | Nichts; keine Datenbank |
| LiteLLM Python 1.110.2 | Einen Master-Schlüssel aus seiner Konfiguration | Nichts; keine Datenbank |
| Bifrost 2.6.0 | Keinen Schlüssel | Nichts; Logging und Stores aus |
| Portkey 1.15.2 | Keinen Schlüssel | Nichts; Logging aus |

## Wie es lief

- **Maschine:** eine DigitalOcean CPU-Optimized c-4 mit 4 dedizierten vCPUs (Intel Xeon Platinum 8168, 2,7 GHz), 8 GB Speicher, Ubuntu 24.04 und Docker, am 25. November 2026. In der Region der früheren Läufe wurde c-4 nicht mehr angeboten, deshalb steht diese Maschine in einer anderen; derselbe Treiber schaffte dort direkt zum Mock über ein Viertel weniger Anfragen pro Sekunde, und die Zahlen jedes Gateways liegen niedriger.
- **Kerne:** Jedes Gateway lief in Docker im Host-Netzwerk, fest auf zwei Kernen. Mock, Lasttreiber und Kaesos Datenbanken liefen auf den anderen beiden.
- **Benchmark:** `BerriAI/ai-gateway-bench` bei `44365ca`, Mock-Anbieter und Treiber unverändert: `load-probe` für Anfragen, `run_http_bench.py` für Streams und `session_runner.py` für Sitzungen.
- **Mock-Anbieter:** das erste Token nach 20 ms, dann 40 Tokens im Abstand von 5 ms. Im Kapazitätslauf antwortet er sofort.
- **Last:** 1, 4, 16 und 64 Clients mit 1.000, 2.000, 4.000 und 16.000 Anfragen pro Runde.
- **Runden:** drei, die Gateways in jeder Runde neu zufällig geordnet. Jedes Gateway wurde direkt nach einem direkten Lauf derselben Form gemessen.
- **CPU und Speicher:** jeder Prozess im Container des Gateways, viermal pro Sekunde gelesen, und ebenso seine Datenbanken.

Die Images liefen mit diesen Digests:

```text
ghcr.io/logicplanes/kaeso-gateway:0.7.3@sha256:e070906766362fc298babb51d2d0ee9a60deb07cbf30aac2e408f6d193f91553
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
curl -fsSLO https://kaeso.app/benchmarks/runs/0.7.3/harness.tar.gz
tar -xzf harness.tar.gz
bash kaeso-gateway-bench/setup.sh
cd /root
ai-gateway-bench/.venv/bin/python run.py
ai-gateway-bench/.venv/bin/python run_capacity.py
# Kaeso 0.7.2 noch einmal, für den Abschnitt oben
ai-gateway-bench/.venv/bin/python run072.py kaeso kaeso-logs
ai-gateway-bench/.venv/bin/python cap072.py kaeso kaeso-logs
```

Drei Runden aller Gateways dauern etwa 90 Minuten, der Kapazitätslauf weitere 30. Die Ergebnisse landen in `/root/out` und `/root/cap`, die von 0.7.2 in `/root/out072` und `/root/cap072`. Absolute Zahlen ändern sich von Maschine zu Maschine; vergleichen Sie Gateways aus demselben Lauf.

## Nächste Schritte

- [Anforderungen](requirements.md): Bemessen Sie den Server für den Verkehr, den Sie erwarten.
- [Mehrere Gateways](scaling.md): Betreiben Sie weitere Gateways, wenn eines nicht reicht.
- [Gateway-Benchmark](../development/benchmark.md): Messen Sie Kaeso allein mit Streams, Abbrüchen und langsamen Apps.
