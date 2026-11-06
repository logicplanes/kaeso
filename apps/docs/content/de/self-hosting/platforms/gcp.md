---
title: Google Cloud
description: Betreiben Sie Kaeso auf Google Cloud, als Compute-Engine-Server mit Compose oder auf Cloud Run mit Load Balancer, Cloud SQL und Memorystore.
order: 4
---

`deploy/gcp/` im Repository enthält zwei Terraform-Konfigurationen, die auch OpenTofu anwendet. Der schnelle Server ist das Compose-Paket auf einer virtuellen Maschine, wie auf Ihrem eigenen Server. Die verwaltete Konfiguration ist Kaeso auf Cloud Run mit den Datenspeichern von Google, für Installationen, denen eine Maschine nicht mehr reicht.

## Schneller Server

`deploy/gcp/quick-server/` legt eine Compute-Engine-Instanz mit Ubuntu 24.04 an, eine statische Adresse, Firewall-Regeln für HTTP, HTTPS und HTTP/3, SSH aus Adressbereichen Ihrer Wahl und einen optionalen Cloud-DNS-Eintrag. Beim ersten Start führt die Instanz den Installer aus, der Docker installiert, die Einstellungen schreibt und das Compose-Paket mit Caddy, gebündeltem ClickHouse und dem Updater startet ([Installation](../installation.md)).

```bash
cd deploy/gcp/quick-server
tofu init
tofu apply -var project=my-project -var domain=kaeso.example.com -var 'ssh_source_ranges=["203.0.113.0/24"]'
```

| Variable | Was es ist |
| --- | --- |
| `project`, `region`, `zone` | Wo die Instanz läuft; standardmäßig `europe-west3-a` |
| `domain` | Die Domain, auf der Kaeso antwortet; ohne `dns_zone` richten Sie ihren A-Eintrag auf die Ausgabe `public_ip` |
| `kaeso_version` | Ein zu installierendes Release; leer installiert das neueste stabile Release |
| `machine_type`, `disk_size_gb` | Standardmäßig `e2-medium` und 40 GB ([Anforderungen](../requirements.md)) |
| `ssh_source_ranges` | Wer SSH erreichen darf, das `kaeso setup-link` und Upgrades von Hand brauchen |
| `dns_zone` | Eine verwaltete Cloud-DNS-Zone der Domain, um ihren Eintrag anzulegen |

Sobald die Instanz läuft und das Zertifikat ausgestellt ist, führen Sie die Ausgabe `setup_link` aus, um den einmaligen Einrichtungslink auszugeben. Updates laufen wie auf jeder Compose-Installation: aus der Konsole, automatisch oder von Hand ([Upgrades](../upgrades.md)).

## Verwalteter Betrieb

`deploy/gcp/managed/` legt an:

- eine VPC mit einem Subnetz, einem Serverless-VPC-Connector und Private Services Access;
- Cloud SQL für PostgreSQL 17 nur mit privater Adresse und Memorystore für Valkey mit `noeviction`, nur über die VPC erreichbar;
- Secret-Manager-Geheimnisse für die Hauptschlüssel, die PostgreSQL-URL, das SMTP-Passwort und die ClickHouse-URL, gelesen vom Dienstkonto des Gateways;
- Cloud-Run-Dienste für das Gateway (standardmäßig 2 CPUs und 4 GB, mindestens eine warme Instanz, ein Anfrage-Timeout von einer Stunde) und die Konsole, nur über den Load Balancer erreichbar;
- einen globalen externen Application Load Balancer mit verwaltetem Zertifikat, HTTP auf HTTPS umgeleitet und Pfadregeln, die `/v1/*`, `/platform/*`, `/mcp`, `/mcp/*`, `/oauth/*` und `/.well-known/oauth-*` an das Gateway und alles andere an die Konsole senden, mit einem Backend-Timeout von einer Stunde für lange Streams;
- einen optionalen Cloud-DNS-Eintrag.

```bash
cd deploy/gcp/managed
tofu init
tofu apply -var project=my-project -var domain=kaeso.example.com \
  -var "master_keys=$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)" \
  -var setup_email=owner@example.com -var smtp_host=smtp.example.com -var smtp_user=kaeso -var smtp_password=secret \
  -var 'smtp_from=Kaeso <kaeso@example.com>'
```

| Variable | Was es ist |
| --- | --- |
| `project`, `region`, `name` | Wo alles läuft und das Präfix jeder Ressource |
| `domain`, `dns_zone` | Die Domain und eine Cloud-DNS-Zone, um ihren Eintrag anzulegen |
| `master_keys` | Die Hauptschlüssel; die Konfiguration hält sie im Secret Manager, und Sie bewahren eine Kopie in einem Passwort-Manager auf |
| `setup_email` | Wohin das Gateway den einmaligen Einrichtungslink sendet, bis der erste Benutzer existiert |
| `clickhouse_url` | Eine ClickHouse-HTTP-URL mit Benutzer und Passwort, etwa ClickHouse Cloud; leer lässt Anfrageprotokolle aus |
| `smtp_…` | Ihr SMTP-Server ([E-Mail](../email.md)) |
| `gateway_cpu`, `gateway_memory`, `gateway_min_instances`, `gateway_max_instances` | Größe des Gateways und wie viele Instanzen laufen; sie teilen Limits, Caches und Routen-Abklingzeiten über Valkey |
| `database_tier`, `database_disk_gb`, `cache_node_type` | Die Größen der Datenspeicher |

Memorystore wird ohne Passwort und ohne TLS erreicht, nur über das private Netz, denn das Gateway kann weder Googles IAM-Token noch seine private Zertifizierungsstelle vorweisen. Das Gateway vertraut `X-Forwarded-For` aus den Proxy-Bereichen des Load Balancers und Cloud Runs eigenem Frontend und hält seine Metriken auf seiner Loopback-Adresse. Das verwaltete Zertifikat wird ausgestellt, sobald die Domain auf die Adresse des Load Balancers zeigt, was bis zu einer Stunde dauert.

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `main.tf`. Wenden Sie die neue Konfiguration an; Cloud Run legt neue Revisionen an und verschiebt den Verkehr, sobald sie gesund sind, und das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Zum absichtlichen Zurückrollen wenden Sie die Konfiguration des vorherigen Releases an oder verschieben den Verkehr in Cloud Run auf die vorherige Revision; ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt.

## Worauf das beruht

Beide Konfigurationen bestehen `tofu fmt` und `tofu validate` mit dem Google-Provider, was CI ausführt. Gateway- und Konsolen-Image wurden hinter einem pfadweiterleitenden Edge lokal mit Docker getestet ([was geprüft wurde](index.md#was-geprüft-wurde)). Keine der beiden Konfigurationen wurde bisher in einem Google-Cloud-Projekt angewendet; wenden Sie die verwaltete zuerst in einem Wegwerf-Projekt an und prüfen Sie die Proxy-Bereiche, die das Gateway in `X-Forwarded-For` sieht, die Einrichtungs-E-Mail, eine gestreamte Antwort, die länger als eine Minute dauert, und ein Update.

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie Cloud SQL und bewahren Sie den Hauptschlüssel außerhalb von Google Cloud auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die Cloud Run und der Load Balancer nutzen.
