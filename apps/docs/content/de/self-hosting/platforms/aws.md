---
title: AWS
description: Betreiben Sie Kaeso auf AWS, als EC2-Server mit dem Compose-Paket oder auf ECS Fargate mit Application Load Balancer, RDS und ElastiCache.
order: 4
---

`deploy/aws/` im Repository enthält zwei CloudFormation-Vorlagen. Der schnelle Server ist das Compose-Paket auf einer Instanz, wie auf Ihrem eigenen Server. Der verwaltete Stack ist Kaeso auf ECS Fargate mit den Datenspeichern von AWS, für Installationen, denen eine Maschine nicht mehr reicht.

## Schneller Server

[Auf AWS starten](https://console.aws.amazon.com/cloudformation/home#/stacks/create/review?templateURL=https%3A%2F%2Fkaeso-cloudformation.s3.amazonaws.com%2Flatest%2Fquick-server.yaml&stackName=kaeso) öffnet die veröffentlichte Vorlage für den schnellen Server in CloudFormation. Dieselbe Datei liegt unter `deploy/aws/quick-server.yaml` im Repository. Jedes Release lädt beide Vorlagen nach `https://kaeso-cloudformation.s3.amazonaws.com/vX.Y.Z/` hoch, ein stabiles Release auch nach `latest/` ([Releases](../../development/releasing.md)).

`deploy/aws/quick-server.yaml` legt eine Graviton-Instanz mit Ubuntu 24.04 an, eine Elastic IP, eine Security Group für HTTP, HTTPS und HTTP/3, SSH aus einem Adressbereich Ihrer Wahl und einen optionalen Route-53-Eintrag. Beim ersten Start führt die Instanz den Installer aus, der Docker installiert, die Einstellungen schreibt und das Compose-Paket mit Caddy, gebündeltem ClickHouse und dem Updater startet ([Installation](../installation.md)).

```bash
aws cloudformation deploy --stack-name kaeso --template-file deploy/aws/quick-server.yaml \
  --parameter-overrides Domain=kaeso.example.com KeyName=my-key AllowedSshCidr=203.0.113.0/24 HostedZoneId=Z0123456789
```

| Parameter | Was es ist |
| --- | --- |
| `Domain` | Die Domain, auf der Kaeso antwortet; ohne `HostedZoneId` richten Sie ihren A-Eintrag auf die Ausgabe `PublicIp` |
| `KaesoVersion` | Ein zu installierendes Release; leer installiert das neueste stabile Release |
| `InstanceType`, `DiskSizeGb` | Standardmäßig `t4g.medium` und 40 GB ([Anforderungen](../requirements.md)) |
| `KeyName`, `AllowedSshCidr` | SSH-Zugang, den `kaeso setup-link` und Upgrades von Hand brauchen |
| `HostedZoneId` | Eine Route-53-Zone der Domain, um ihren Eintrag anzulegen |

Sobald der Stack fertig und das Zertifikat ausgestellt ist, führen Sie die Ausgabe `SetupLink` über SSH aus, um den einmaligen Einrichtungslink auszugeben. Updates laufen wie auf jeder Compose-Installation: aus der Konsole, automatisch oder von Hand ([Upgrades](../upgrades.md)).

## Verwalteter Betrieb

`deploy/aws/managed.yaml` legt an:

- eine VPC mit zwei öffentlichen und zwei privaten Subnetzen und einem NAT-Gateway;
- einen Application Load Balancer mit Ihrem ACM-Zertifikat, HTTP auf HTTPS umgeleitet, Pfadregeln, die `/v1/*`, `/platform/*`, `/mcp`, `/mcp/*`, `/oauth/*` und `/.well-known/oauth-*` an das Gateway und alles andere an die Konsole senden, und einem Leerlauf-Timeout von 300 Sekunden für lange Streams;
- einen ECS-Cluster mit einem Gateway-Dienst (standardmäßig 2 vCPUs und 4 GB, 2 Tasks) und einem Konsolen-Dienst, auf Fargate mit Graviton, in den privaten Subnetzen;
- RDS PostgreSQL 18 und ElastiCache Valkey mit `noeviction`, TLS und einem AUTH-Token, nur vom Gateway erreichbar;
- Secrets-Manager-Geheimnisse für die Hauptschlüssel, das Datenbankpasswort, das Valkey-Token, das SMTP-Passwort und die ClickHouse-URL;
- CloudWatch-Protokollgruppen und einen optionalen Route-53-Alias.

```bash
aws cloudformation deploy --stack-name kaeso --template-file deploy/aws/managed.yaml --capabilities CAPABILITY_IAM \
  --parameter-overrides Domain=kaeso.example.com CertificateArn=arn:aws:acm:eu-central-1:123456789012:certificate/example \
  MasterKeys="$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)" \
  SetupEmail=owner@example.com SmtpHost=smtp.example.com SmtpUser=kaeso SmtpPassword=secret SmtpFrom="Kaeso <kaeso@example.com>"
```

| Parameter | Was es ist |
| --- | --- |
| `Domain`, `CertificateArn` | Die Domain und ein ACM-Zertifikat dafür in der Region des Stacks; `HostedZoneId` legt den Alias-Eintrag an |
| `MasterKeys` | Die Hauptschlüssel; der Stack hält sie im Secrets Manager, und Sie bewahren eine Kopie in einem Passwort-Manager auf |
| `SetupEmail` | Wohin das Gateway den einmaligen Einrichtungslink sendet, bis der erste Benutzer existiert |
| `ClickHouseUrl` | Eine ClickHouse-HTTP-URL mit Benutzer und Passwort, etwa ClickHouse Cloud; leer lässt Anfrageprotokolle aus |
| `Smtp…` | Ihr SMTP-Server ([E-Mail](../email.md)) |
| `GatewayCpu`, `GatewayMemory`, `GatewayCount` | Größe des Gateways und wie viele Tasks laufen; sie teilen Limits, Caches und Routen-Abklingzeiten über Valkey |
| `DatabaseInstanceClass`, `DatabaseStorageGb`, `CacheNodeType` | Die Größen der Datenspeicher |

Das Gateway vertraut `X-Forwarded-For` aus der VPC, in der die Knoten des Load Balancers sitzen, und hält seine Metriken auf seiner Loopback-Adresse. Der Load Balancer prüft `/readyz` am Gateway und `/healthz` an der Konsole; ein Gateway-Task bekommt beim Stoppen 75 Sekunden zum Abarbeiten.

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `managed.yaml`. Rollen Sie die neue Vorlage über den Stack aus; ECS ersetzt die Tasks mit einem Circuit Breaker, der bei Fehlern zurückrollt, und das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Zum absichtlichen Zurückrollen rollen Sie die Vorlage des vorherigen Releases aus; ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt.

## Worauf das beruht

Beide Vorlagen bestehen `cfn-lint`, das CI ausführt. Am 2026-11-07 legte die veröffentlichte Vorlage des schnellen Servers in einem leeren AWS-Konto in `us-east-1` einen Stack an und installierte den achten Release-Kandidaten. Der erste Start zeigte einen Besitzfehler: Die Vorlage übernahm `/opt/kaeso` nach der Installation, sodass das Gateway die Master-Key-Datei nicht lesen konnte. Nachdem der Besitz auf der Testinstanz wiederhergestellt war, bestanden Einrichtungslink, HTTPS-Health- und Readiness-Prüfungen, Konsole und API-Pfade, eine gestreamte Antwort, das ClickHouse-Anfrageprotokoll und das Ersetzen eines gefälschten `X-Forwarded-For` ([was geprüft wurde](index.md#was-geprüft-wurde)). Die Vorlage lässt die Datei des Installers jetzt unverändert, und ein Regressionstest lehnt das alte `chown` ab; die korrigierte Vorlage wurde noch nicht erneut aus einem leeren Konto ausgerollt. Der verwaltete Stack wurde nur gelintet; rollen Sie ihn vor dem produktiven Einsatz in einem Wegwerf-Konto aus.

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie RDS und bewahren Sie den Hauptschlüssel außerhalb von AWS auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die der Load Balancer nutzt.
