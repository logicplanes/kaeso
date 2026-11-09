---
title: Azure
description: Betreiben Sie Kaeso auf Azure auf einer virtuellen Maschine mit dem Compose-Paket, per Button Deploy to Azure oder Azure CLI.
order: 6
---

`deploy/azure/` im Repository enthält eine Azure-Resource-Manager-Vorlage für einen schnellen Server: das Compose-Paket auf einer virtuellen Maschine, wie auf Ihrem eigenen Server. `createUiDefinition.json` daneben gestaltet das Formular des Portals für die Vorlage.

## Was die Vorlage anlegt

- ein virtuelles Netz mit einem Subnetz und einer Netzwerksicherheitsgruppe, die nur HTTP, HTTPS und HTTP/3 einlässt, und SSH nur aus einem Adressbereich, den Sie angeben;
- eine statische öffentliche Adresse mit einem DNS-Namen unter `<region>.cloudapp.azure.com`;
- eine virtuelle Maschine mit Ubuntu 24.04, Trusted Launch, Startdiagnose und einer Festplatte mit 40 GB;
- eine Custom-Script-Erweiterung, die beim ersten Start den Installer ausführt. Er installiert Docker, schreibt die Einstellungen und startet das Compose-Paket mit Caddy, gebündeltem ClickHouse und dem Updater ([Installation](../installation.md)).

Das Deployment endet, sobald der Installer fertig ist. Ohne eigene Domain antwortet Kaeso auf dem DNS-Namen der Adresse, und Caddy bezieht sein Zertifikat für diesen Namen.

## Mit dem Button ausrollen

1. Öffnen Sie [Deploy to Azure](https://portal.azure.com/#create/Microsoft.Template/uri/https%3A%2F%2Fraw.githubusercontent.com%2Flogicplanes%2Fkaeso%2Fprod%2Fdeploy%2Fazure%2Fquick-server.json/createUIFormUri/https%3A%2F%2Fraw.githubusercontent.com%2Flogicplanes%2Fkaeso%2Fprod%2Fdeploy%2Fazure%2FcreateUiDefinition.json) und melden Sie sich an. Das Portal liest die Vorlage aus dem Branch `prod`, der das neueste Release enthält.
2. Wählen Sie ein Abonnement, eine neue Ressourcengruppe und eine Region, und füllen Sie die [Parameter](#parameter) aus.
3. Legen Sie das Deployment an und warten Sie, bis es fertig ist, etwa fünf Minuten.
4. Kopieren Sie unter **Outputs** des Deployments `setupLink` und führen Sie es in der [Azure Cloud Shell](https://shell.azure.com) oder mit der Azure CLI aus. Es gibt den einmaligen Einrichtungslink ohne SSH aus; öffnen Sie ihn, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)).
5. Mit eigener Domain richten Sie ihren CNAME-Eintrag auf die Ausgabe `fqdn`, oder einen A-Eintrag auf `publicIp`. Caddy versucht es weiter, bis der Name auflöst, und bezieht dann das Zertifikat.

## Mit der Azure CLI ausrollen

Legen Sie in einem Klon des Repositorys eine Ressourcengruppe an und rollen Sie die Vorlage darin aus:

```bash
az group create --name kaeso --location westeurope
az deployment group create --resource-group kaeso --template-file deploy/azure/quick-server.json \
  --parameters domain=kaeso.example.com adminPasswordOrKey="$(cat ~/.ssh/id_ed25519.pub)" sshSourceAddress=203.0.113.0/24
```

Der Befehl gibt die Ausgaben aus, sobald das Deployment fertig ist. Führen Sie `setupLink` daraus aus wie in Schritt 4 des Buttons.

## Parameter

| Parameter | Was es ist |
| --- | --- |
| `domain` | Die Domain, auf der Kaeso antwortet; leer nutzt den DNS-Namen der Adresse |
| `dnsLabel` | Der DNS-Name der Adresse unter `<region>.cloudapp.azure.com`; leer bildet einen aus der Ressourcengruppe |
| `kaesoVersion` | Ein zu installierendes Release; leer installiert das neueste stabile Release |
| `vmSize`, `diskSizeGb`, `diskType` | Standardmäßig `Standard_B2als_v2` mit 2 vCPUs und 4 GB und eine Standard SSD mit 40 GB ([Anforderungen](../requirements.md)) |
| `adminUsername`, `authenticationType`, `adminPasswordOrKey` | Der Administrator des Servers und ein öffentlicher SSH-Schlüssel oder ein Passwort, für SSH und die serielle Konsole |
| `sshSourceAddress` | Ein Adressbereich, der SSH erreichen darf; leer hält Port 22 geschlossen |
| `location` | Die Region; standardmäßig die der Ressourcengruppe |

Die Vorlage enthält kein Geheimnis von Kaeso: Der Installer erzeugt den Hauptschlüssel und die Passwörter auf dem Server. Bewahren Sie eine Kopie von `/opt/kaeso/master-keys` getrennt von Ihren Backups auf; `setupLink` mit `cat /opt/kaeso/master-keys` als Skript gibt sie aus ([Backups und Hauptschlüssel](../backups.md)).

## Aktualisieren

Updates laufen wie auf jeder Compose-Installation: aus der Konsole, automatisch oder von Hand ([Upgrades](../upgrades.md)). Die Vorlage legt nur den Server an; sie erneut auszurollen aktualisiert Kaeso nicht.

## Grenzen

- Die Vorlage ist nur ein schneller Server. Das Repository hat keine Vorlage für Azure Container Apps mit Azure Database for PostgreSQL und einem verwalteten Cache.
- Die Größen sind x64-Größen. Für eine Arm64-Größe rollen Sie Ubuntu 24.04 für Arm64 aus und führen den Installer von Hand aus.

## Worauf das beruht

Die Vorlage folgt den Resource-Manager-Referenzen für virtuelle Maschinen, die Custom-Script-Erweiterung, öffentliche Adressen und Netzwerksicherheitsgruppen sowie Microsofts Seite zum Button Deploy to Azure, gelesen am 9. November 2026. Sie besteht alle 49 Tests des [ARM Template Test Toolkit](https://github.com/Azure/arm-ttk), die auch `createUiDefinition.json` prüfen. Sie wurde noch nicht auf Azure ausgerollt, und `az deployment group validate` braucht ein angemeldetes Konto, lief also nicht. Rollen Sie sie zuerst in einer Wegwerf-Ressourcengruppe aus und prüfen Sie den Einrichtungslink, eine gestreamte Antwort und die Client-Adresse in einem Anfrageprotokoll ([was geprüft wurde](index.md#was-geprüft-wurde)).

## Nächste Schritte

- [Installation](../installation.md): Was der Installer auf dem Server einrichtet und wie Sie es ändern.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie den Server und bewahren Sie den Hauptschlüssel außerhalb von Azure auf.
- [Upgrades](../upgrades.md): Aktualisieren Sie Kaeso aus der Konsole oder von Hand.
