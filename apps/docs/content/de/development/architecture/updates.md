---
title: Updates
description: Wie das Gateway von Releases erfährt, was ein Release für den Updater mitbringt und wie der Updater es prüft und einspielt.
order: 5
---

```text
GitHub releases API ◀── gateway (update_check job, instance/updates/*) ──▶ PostgreSQL
                              │  bearer token, internal network "updates"
                              ▼
                           updater ──▶ github.com (release.json, compose.yaml), api.github.com (attestations), ghcr.io
                              │  internal network "docker"
                              ▼
                         docker-proxy ──▶ /var/run/docker.sock
                              │
updater ──▶ maintenance volume ◀── Caddy (updating page, JSON 503s)
```

## Das Gateway

`services/updates.rs` und der Job `update_check` suchen nach Releases: Alle fünf Minuten fragt der Job, ob die nächste Prüfung fällig ist (`update_settings.next_check_at`), und eine Prüfung lädt `releases_url?per_page=50` mit `If-None-Match`. Eine geänderte Liste ersetzt `update_releases` in einer Transaktion; für jedes Release, das neuer als die laufende Version ist, wird sein `release.json` geladen und gespeichert, oder der Grund, warum das nicht ging. Die nächste Prüfung folgt 12 Stunden später plus einem zufälligen Teil einer Stunde, nach einem Fehler frühestens eine Stunde später, oder noch später, wenn `Retry-After` oder `x-ratelimit-reset` von GitHub das sagen. Ein wiederholter Fehler wird nur auf Debug-Ebene geloggt.

`kaeso_update::policy::plan` wählt, was angeboten wird: das neueste Release des Kanals, das neuer als die laufende Version ist, und ob es von Hand eingespielt werden muss: eine neue Hauptversion, `manual: true` im Manifest eines übersprungenen Releases (Vorabversionen eingeschlossen) oder gar kein Manifest. Die Platform API unter `instance/updates` ist nur für Instanz-Admins; jede Änderung wird protokolliert (`instance.update_check`, `instance.update_settings`, `instance.update_start`, `instance.update_finish`), und `apply` verlangt eine Anmeldung in den letzten 15 Minuten.

`update_jobs` enthält jedes Update; ein eindeutiger Index erlaubt immer nur eine Zeile `running`. Solange eine Zeile läuft, fragt das Lesen den Updater nach ihrem Stand und hält das Ende einmal fest, mit `instance.update_finish`, egal welches Gateway sie zuerst liest: meist das neue, nach der Umstellung. Ein Job, den der Updater nicht kennt oder zu dem er seit einer Stunde nicht antwortet, endet als `interrupted` oder `updater_unreachable`.

## Was ein Release mitbringt

Der Release-Workflow baut drei Images, `kaeso-gateway`, `kaeso-console` und `kaeso-updater`, attestiert ihre Build-Provenance mit `actions/attest-build-provenance` und hängt `release.json` an (`scripts/release-manifest.mjs`), dessen Provenance er ebenfalls attestiert:

```json
{
  "schema": 1,
  "version": "1.4.2",
  "commit": "5388ad0b98aa3d957fea99e200259842bf746680",
  "manual": false,
  "updater_protocol": 1,
  "images": {
    "gateway": { "name": "ghcr.io/logicplanes/kaeso-gateway", "digest": "sha256:…" },
    "console": { "name": "ghcr.io/logicplanes/kaeso-console", "digest": "sha256:…" },
    "updater": { "name": "ghcr.io/logicplanes/kaeso-updater", "digest": "sha256:…" }
  },
  "compose": { "name": "compose.yaml", "sha256": "…" }
}
```

`manual` ist wahr, wenn die Eingabe **manual** des Release-Workflows es sagt oder wenn sich `compose.yaml` von der des vorigen Releases in mehr als den Image-Versionen unterscheidet. `kaeso_update::manifest` liest es, lehnt ein anderes Schema ab und prüft jeden Namen, jeden Digest und den Commit.

## Der Updater

`crates/kaeso-updater` ist ein kleiner axum-Server auf Port 8081: `GET /healthz` ohne Token und `GET /v1/status`, `POST /v1/updates` (`{ "id", "version" }`) und `GET /v1/updates/{id}` mit `Authorization: Bearer` und dem gemeinsamen Token. Er führt immer nur ein Update aus und hält es in `/state/job.json`, das nach jedem Schritt atomar geschrieben wird.

Ein Update (`engine.rs`):

1. lädt `release.json` und `compose.yaml` aus dem Release, prüft das Manifest, findet die Container von Gateway und Konsole seines eigenen Compose-Projekts (aus den Labels seines eigenen Containers), liest die laufende Version aus `org.opencontainers.image.version` des Gateway-Images, lehnt eine ältere, gleiche oder neue Hauptversion ab und prüft, dass die lokale `compose.yaml` mit den neuen Image-Versionen Byte für Byte der des Releases gleicht;
2. lädt die drei Images über ihren Digest und prüft `release.json` und jedes Image (`verify.rs`): die Attestierungen von `api.github.com/repos/logicplanes/kaeso/attestations/sha256:…`, jede ein Sigstore-Bundle, geprüft mit [sigstore-rs](https://github.com/sigstore/sigstore-rs) gegen die Vertrauensbasis in `data/trusted_root.json` (die Zertifikatskette bis Fulcio, der SCT, die Identität `https://github.com/logicplanes/kaeso/.github/workflows/release.yml@refs/heads/prod` des Ausstellers `https://token.actions.githubusercontent.com`, die DSSE-Signatur, der Eintrag im Transparenzlog), dazu der signierte Zeitstempel des Logs, den der Updater selbst mit dem Rekor-Schlüssel prüft, und die in-toto-Aussage: SLSA-Provenance v1, Name und Digest des Gegenstands, der Workflow und der Commit aus `release.json`. Bei Images ist der Gegenstand der über den Digest von `ghcr.io` geladene Image-Index; das Versions-Label des geladenen Images muss dem Release entsprechen;
3. schreibt die Wartungsmarkierung und stoppt dann für die Konsole und danach für das Gateway den Container, benennt ihn in `…-previous` um, legt den neuen mit den Einstellungen des alten an (`clone.rs`: alles, was vom alten Image kam, fällt weg, sodass Umgebung, Befehl und Healthcheck des neuen Images gelten) und startet ihn;
4. wartet auf jeden Healthcheck: 2 Minuten für die Konsole, 10 für die Migrationen des Gateways; ein Absturz, ein Neustart oder `unhealthy` schlägt sofort fehl;
5. schreibt `compose.yaml` an Ort und Stelle, taggt die Images, entfernt die vorigen Container und löscht die Markierung.

Ein Fehler vor der Umstellung ändert nichts (`failed`). Ein Fehler danach startet die vorigen Container in umgekehrter Reihenfolge wieder und wartet auf sie (`rolled_back`, oder `rollback_failed` mit dem Grund). Beim Start wird ein Update, das laut Datei läuft, zurückgesetzt, oder abgeschlossen, wenn nur das Aufräumen fehlte.

Der Updater erreicht Docker nur unter `http://docker-proxy:2375/v1.44/`, dessen Freigabeliste in `compose.yaml` jeden Pfad nennt, den er nutzt.

## Die Edge

Caddy liest die Wartungsmarkierung aus dem Volume `maintenance`. Solange sie existiert, bekommt jede Anfrage sofort die Wartungsantwort; sonst tut es ein `502`, `503` oder `504` von Gateway oder Konsole. `/kaeso-maintenance.json` liefert die Markierung, `{"state":"updating","step":"migrating"}`, damit die Seite den Schritt nennen kann, ohne Versionen zu nennen. Die Seite steht inline in `compose.yaml`, mit einer Content-Security-Policy, die genau ihren eigenen Stil und ihr eigenes Skript per Hash erlaubt. `crates/kaeso-server/tests/edge.rs` prüft, dass die JSON-Antworten dem entsprechen, was `ApiError` und `kaeso_codecs::dispatch::encode_error` für dieselbe Meldung erzeugen, dass die Hashes stimmen und dass die Seite nichts von anderswo lädt.

## Tests

| Wo | Was |
| --- | --- |
| `crates/kaeso-update` | Versions-Tags, Kanäle, der Plan mit Hauptversionen, Schritten von Hand und Vorabversionen (mit einem Property-Test), das Manifest, das Protokoll |
| `crates/kaeso-server/tests/updates.rs` | Gegen ein Mock-GitHub und einen Mock-Updater: bedingte Anfragen, Fehler und ihre Wartezeiten, Installationen ohne Internet, Rechte, das Audit-Log, die Sperre auf ein Update, Fortschritt und jedes Ende |
| `crates/kaeso-updater/tests/verify.rs` | Die echten Attestierungen von Kaeso 0.1.0 bestehen die Prüfung; ein anderes Image, ein anderer Commit, Workflow, eine andere Signatur oder ein anderer Logeintrag nicht |
| `crates/kaeso-updater/tests/engine.rs`, `server.rs` | Gegen eine nachgebaute Docker-API: ein vollständiges Update, jede Ablehnung, Zurücksetzen, ein fehlgeschlagenes Zurücksetzen, Neustarts mitten im Update, das Token und die API |
| `e2e/install` | Die Installation mit dem Profil `updater`: Das Gateway erreicht den Updater, der Updater erreicht Docker nur über den Proxy, und die Antworten von Caddy in jedem Format, während Kaeso weg ist, über die offiziellen SDKs |
| `apps/console` | Die Seite Updates in jedem Zustand, in Unit-Tests und gegen die Mock-Releases und den Mock-Updater des Stacks; die Update-Seite in drei Sprachen, hell und dunkel, mit axe |
