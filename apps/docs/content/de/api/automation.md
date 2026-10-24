---
title: Mit der Platform API automatisieren
description: Ein Projekt, ein Modell und einen Schlüssel für eine App per Skript einrichten und Nutzung und Protokolle lesen.
order: 4
---

Richten Sie alles, was eine App braucht, per Skript ein: ein Projekt, einen Anbieter-Schlüssel, ein Modell und den eigenen API-Schlüssel der App. Die Anfragen verwenden einen Plattform-Schlüssel, der mit den Rechten der Rolle Admin in der ganzen Organisation handelt.

## Voraussetzungen

- Ein Plattform-Schlüssel aus **Plattform-Schlüssel** in den Einstellungen der Organisation, in der Umgebungsvariable `KAESO_PLATFORM_KEY` ([Plattform-Schlüssel](../console/api-keys.md#plattform-schlüssel)).
- Die ID der Organisation, `org_…`, aus der Adresse der Konsole.
- Ein Anthropic-API-Schlüssel in `ANTHROPIC_API_KEY`, sofern die Organisation noch keinen hat.
- `curl` und `jq`.

Legen Sie die Adresse der Organisation in einer Variable ab, mit der ID Ihrer Organisation anstelle von `org_…`:

```bash
ORG_URL=https://kaeso.example.com/platform/v1/orgs/org_…
```

## Projekt erstellen

Erstellen Sie das Projekt und behalten Sie seine ID:

```bash
PROJECT=$(curl -s "$ORG_URL/projects" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"name": "Support bot"}' | jq -r .id)
```

`PROJECT` enthält nun die ID des neuen Projekts, `proj_…`.

## Anbieter-Schlüssel hinzufügen

Fügen Sie den Anthropic-Schlüssel der Organisation hinzu. Überspringen Sie diesen Schritt, wenn die Organisation schon einen hat.

```bash
curl -s "$ORG_URL/credentials" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d "$(jq -n --arg secret "$ANTHROPIC_API_KEY" \
    '{provider: "anthropic", name: "Production", secret: $secret}')"
```

Der erste Schlüssel eines Anbieters wird sein Standard, den jedes Projekt verwendet, solange Sie ihm keinen anderen zuweisen. Die Antwort zeigt nur die letzten vier Zeichen des Schlüssels, und Kaeso zeigt das Secret nie wieder.

## Modell hinzufügen

Fügen Sie Claude Haiku dem Katalog des Projekts hinzu:

```bash
curl -s "$ORG_URL/projects/$PROJECT/models" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"provider": "anthropic", "model_id": "claude-haiku-4-5"}'
```

Kaeso übernimmt Preise, Limits und Fähigkeiten des Modells von models.dev, und die Antwort zeigt sie.

## Schlüssel der App erstellen

Geben Sie der App ein Dienstkonto mit der Rolle Entwickler, damit ihr Schlüssel nicht an einer Person hängt. Erstellen Sie dann einen Schlüssel dafür, der nach 90 Tagen abläuft:

```bash
ACCOUNT=$(curl -s "$ORG_URL/projects/$PROJECT/service-accounts" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"name": "support-bot", "role": "developer"}' | jq -r .id)

KAESO_API_KEY=$(
  curl -s "$ORG_URL/projects/$PROJECT/service-accounts/$ACCOUNT/keys" \
    -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
    -H "content-type: application/json" \
    -d '{"name": "support-bot", "expires_in_days": 90}' | jq -r .secret
)
```

`KAESO_API_KEY` enthält nun den Schlüssel der App, `sk-kso-…`. Kaeso zeigt das Secret eines Schlüssels nur einmal, legen Sie es also dort ab, wo die App es liest.

## Modell aufrufen

Senden Sie eine Anfrage mit dem neuen Schlüssel, so wie die App es tun wird:

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

Die Antwort ist eine Messages-Antwort von Claude. Wie die App ihr SDK einrichtet, zeigt die [Modell-API](model-api.md).

## Nutzung und Protokolle lesen

Lesen Sie die Nutzung des Projekts nach Modell und seine zehn neuesten Anfragen:

```bash
curl -s "$ORG_URL/projects/$PROJECT/usage?group_by=model" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
curl -s "$ORG_URL/projects/$PROJECT/logs?limit=10" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
```

Eine Anfrage erscheint innerhalb weniger Sekunden in den Protokollen. Die Nutzung enthält Kosten, wenn der Aufrufer das Recht `costs.view` hat, und nimmt diese Query-Parameter:

- `group_by`: `day` (Standard), `week`, `month`, `model`, `key` oder `member`.
- `from` und `to`: Tage in UTC, etwa `2026-08-15`. Nach Tagen gruppiert liegen sie höchstens 400 Tage auseinander.
- `usage/span` antwortet mit dem ersten und letzten Tag mit Nutzung, damit Sie einen langen Zeitraum nach Woche oder Monat lesen können.

Hängen Sie die ID eines Protokolleintrags an, um eine einzelne Anfrage zu lesen, etwa `logs/req_…`. Er enthält den Inhalt der Anfrage, wenn das Projekt Inhalte protokolliert.

## Stattdessen einen Projektschlüssel verwenden

Ein API-Schlüssel mit Plattformzugriff funktioniert innerhalb seines eigenen Projekts genauso, mit den Rechten seines Mitglieds oder Dienstkontos. Senden Sie ihn als `Authorization: Bearer sk-kso-…`. Er kann mit `models.manage` Modelle hinzufügen und mit `usage.view` und `logs.metadata.view` Nutzung und Protokolle lesen.

Projekte erstellen und Anbieter-Schlüssel hinzufügen sind Rechte der Organisation, die ein Projektschlüssel nie hat. Ein Schlüssel, der einem Mitglied gehört, kann mit `POST orgs/{org}/projects/{project}/keys` weitere Schlüssel für dieses Mitglied erstellen. [API-Schlüssel](../console/api-keys.md) erklärt, wie Sie einem Schlüssel Plattformzugriff geben.

## Stattdessen eine Sitzung verwenden

Melden Sie sich mit einem Passwort an, legen Sie das Sitzungs-Cookie in einer Datei ab, und senden Sie `Origin` mit jeder Anfrage, die etwas ändert:

```bash
JAR=$(mktemp)

curl -s -c "$JAR" https://kaeso.example.com/platform/v1/auth/password \
  -H "Origin: https://kaeso.example.com" \
  -H "content-type: application/json" \
  -d '{
    "email": "owner@example.com",
    "password": "correct horse battery staple"
  }'
```

Die Anfragen oben funktionieren dann mit `-b "$JAR" -H "Origin: https://kaeso.example.com"` anstelle des Headers `Authorization`. Eine Sitzung handelt mit Ihren eigenen Rechten und erstellt mit `POST orgs/{org}/projects/{project}/keys` Schlüssel für Ihren eigenen Benutzer.

Ist die Zwei-Faktor-Authentifizierung aktiv, antwortet `auth/password` mit 202 und einer `challenge`. Senden Sie sie an `POST auth/totp` mit `{"challenge": "…", "code": "123456"}` oder mit `recovery_code` anstelle von `code`. Die Anmeldung mit Passwort funktioniert nur, solange sie für die Installation eingeschaltet ist, was der Standard ist. In einer neuen Installation registriert sich der erste Benutzer über den Einrichtungslink von `kaeso setup-link` ([Installation](../self-hosting/installation.md)).

## Nächste Schritte

- [Platform API](platform-api.md): Jede Route, die Konventionen, Fehler und Rechte nachschlagen.
- [API-Schlüssel](../console/api-keys.md): Einen Schlüssel auf die Modelle und Raten beschränken, die eine App braucht.
- [Limits und Budgets](../console/limits-and-budgets.md): Anfragen, Token und Ausgaben eines Projekts begrenzen.
