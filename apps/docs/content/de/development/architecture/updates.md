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

`services/updates.rs` und der Job `update_check` suchen nach Releases: Alle fünf Minuten fragt der Job, ob die nächste Prüfung fällig ist (`update_settings.next_check_at`), und eine Prüfung lädt `releases_url?per_page=50` mit `If-None-Match`. Eine geänderte Liste ersetzt `update_releases` in einer Transaktion; für jedes Release, das neuer als die laufende Version ist, wird sein `release.json` geladen und gespeichert, oder der Grund, warum das nicht ging. Pro Prüfung werden höchstens zwölf Manifeste geladen, vier gleichzeitig, die neuesten zuerst; die übrigen warten auf die nächste Prüfung, die dann ohne `If-None-Match` fragt. Ein Manifest wird nur von `https://github.com/logicplanes/kaeso/releases/download/` geladen, oder vom Origin von `updates.releases_url`, wenn diese auf einen Spiegel zeigt, und der Client folgt nie einer Umleitung von HTTPS auf HTTP. Die nächste Prüfung folgt 12 Stunden später plus einem zufälligen Teil einer Stunde, nach einem Fehler frühestens eine Stunde später, oder noch später, wenn `Retry-After` oder `x-ratelimit-reset` von GitHub das sagen; bis dahin hält `update_settings.blocked_until` auch **Jetzt prüfen** vom Fragen ab. Ein wiederholter Fehler wird nur auf Debug-Ebene geloggt.

`kaeso_update::policy::plan` wählt, was angeboten wird: das neueste Release des Kanals, das neuer als die laufende Version ist, und ob es von Hand eingespielt werden muss: eine neue Hauptversion, `manual: true` im Manifest eines übersprungenen Releases (Vorabversionen eingeschlossen) oder gar kein Manifest. Die Platform API unter `instance/updates` ist nur für Instanz-Admins; jede Änderung wird protokolliert (`instance.update_check`, `instance.update_settings`, `instance.update_start`, `instance.update_finish`), und `apply` verlangt eine Anmeldung in den letzten 15 Minuten.

`update_jobs` enthält jedes Update; ein eindeutiger Index erlaubt immer nur eine Zeile `running`. `apply` schreibt die Zeile, bevor es den Updater aufruft. Lehnt der Updater ab (jede HTTP-Antwort außer `202`) oder lässt er sich nicht verbinden, endet die Zeile sofort als `failed`, mit `updater_refused` oder `updater_unreachable`; ging die Anfrage hinaus, kam aber keine klare Antwort zurück, bleibt die Zeile `running`, und die Aufzeichnung des Updaters entscheidet. Solange eine Zeile läuft, fragt das Lesen den Updater nach ihrem Stand und hält das Ende einmal fest, mit `instance.update_finish`, egal welches Gateway sie zuerst liest: meist das neue, nach der Umstellung. Ein Job, den der Updater zwei Minuten nach seinem Start noch nicht kennt, endet als `interrupted`; einer, zu dem der Updater seit einer Stunde nicht antwortet, als `updater_unreachable`.

## Was ein Release mitbringt

Der Release-Workflow baut drei Images, `kaeso-gateway`, `kaeso-console` und `kaeso-updater`, und attestiert ihre Build-Provenance mit `actions/attest-build-provenance`. Der Job `release-manifest` schreibt `release.json` (`scripts/release-manifest.mjs`); der Job `attest-manifest`, außer den Image-Manifesten der einzige mit `id-token: write`, sieht nichts als diese Datei und attestiert sie; der Job `github-release` prüft dann `release.json` und jedes Image mit `gh attestation verify` gegen diesen Workflow auf `refs/heads/prod` und den Commit, den `release.json` nennt, und legt das Release nur an, wenn alle Prüfungen bestehen:

```json
{
  "schema": 1,
  "version": "1.4.2",
  "commit": "5388ad0b98aa3d957fea99e200259842bf746680",
  "manual": false,
  "manual_since": "1.3.0",
  "updater_protocol": 1,
  "images": {
    "gateway": { "name": "ghcr.io/logicplanes/kaeso-gateway", "digest": "sha256:…" },
    "console": { "name": "ghcr.io/logicplanes/kaeso-console", "digest": "sha256:…" },
    "updater": { "name": "ghcr.io/logicplanes/kaeso-updater", "digest": "sha256:…" }
  },
  "compose": { "name": "compose.yaml", "sha256": "…" },
  "migrations": { "postgres": "20261017120001_updates", "clickhouse": "0002_usage_rollups" }
}
```

`commit` ist der Commit, auf dem der Release-Workflow lief und den die Attestierungen nennen, nicht der Release-Commit des Tags darüber; das Skript lehnt jeden anderen ab. `manual` ist wahr, wenn die Eingabe **manual** des Release-Workflows es sagt oder wenn sich `compose.yaml` von der des vorigen Releases in mehr als den Image-Versionen unterscheidet. `manual_since` nennt das neueste Release bis zu diesem mit `manual: true`, übernommen aus dem `release.json` des vorigen Releases. `migrations` nennt die letzte PostgreSQL- und ClickHouse-Migration des Releases. `kaeso_update::manifest` liest es, lehnt ein anderes Schema ab und prüft jeden Namen, jeden Digest, die Migrationen, `manual_since` und den Commit.

## Der Updater

`crates/kaeso-updater` ist ein kleiner axum-Server auf Port 8081: `GET /healthz` ohne Token und `GET /v1/status`, `POST /v1/updates` (`{ "id", "version" }`) und `GET /v1/updates/{id}` mit `Authorization: Bearer` und dem gemeinsamen Token. Er führt immer nur ein Update aus und hält es in `/state/job.json`, das nach jeder Änderung atomar geschrieben wird; ein Update, dessen erster Stand sich nicht schreiben lässt, wird mit `503` abgelehnt.

Ein Update (`engine.rs`):

1. **Prüfen.** Es lädt `release.json` und prüft sie vor allem anderen, lehnt ein Updater-Protokoll ab, das es nicht spricht, findet die Container von Gateway und Konsole seines eigenen Compose-Projekts (aus den Labels seines eigenen Containers; einmalige Container von `docker compose run` zählen nicht, und ein von einem früheren Update übrig gebliebener Container `…-previous` hält es auf), liest die laufende Version aus `org.opencontainers.image.version` des Gateway-Images, lehnt eine ältere, gleiche oder neue Hauptversion ab sowie ein Release, dessen `manual` oder `manual_since` nach der laufenden Version Schritte von Hand verlangt, prüft, dass die lokale `compose.yaml` mit den neuen Image-Versionen Byte für Byte der des Releases gleicht, und legt eine Kopie davon in `/state` ab, und prüft den Index jedes Images (`verify.rs`): die Attestierungen von `api.github.com/repos/logicplanes/kaeso/attestations/sha256:…`, jede ein Sigstore-Bundle, geprüft mit [sigstore-rs](https://github.com/sigstore/sigstore-rs) gegen die Vertrauensbasis in `data/trusted_root.json` (die Zertifikatskette bis Fulcio, der SCT, die Identität `https://github.com/logicplanes/kaeso/.github/workflows/release.yml@refs/heads/prod` des Ausstellers `https://token.actions.githubusercontent.com`, die DSSE-Signatur, der Eintrag im Transparenzlog), dazu der signierte Zeitstempel des Logs, den der Updater selbst mit dem Rekor-Schlüssel prüft, und die in-toto-Aussage: SLSA-Provenance v1, Name und Digest des Gegenstands, der Workflow und der Commit aus `release.json`. Jede Fulcio-Stelle, jedes CT-Log und jeder Rekor-Schlüssel zählt nur innerhalb seines `validFor` in der Vertrauensbasis, zur Eintragszeit des Logeintrags. Ein Bundle ohne signierten Zeitstempel, wie Rekor v2 sie schreibt, wird mit einer Meldung abgelehnt, die das sagt. Bei Images ist der Gegenstand der über den Digest von `ghcr.io` geladene Image-Index.
2. **Herunterladen.** Es lädt die drei Images über ihren Digest; das Versions-Label jedes geladenen Images muss dem Release entsprechen. Dann lädt es das `release.json` des laufenden Releases, prüft es genauso und vergleicht die Migrationen; unterscheiden sie sich, oder ist dieses Manifest nicht zu haben, gilt das Update als eines, das die Datenbank migriert.
3. **Umstellen.** Es schreibt die Wartungsmarkierung und hält dann für die Konsole und danach für das Gateway den Container fest, stoppt ihn, benennt ihn in `…-previous` um, hält fest, dass es den neuen anlegt, legt ihn mit den Einstellungen des alten an (`clone.rs`: alles, was vom alten Image kam, fällt weg, sodass Umgebung, Befehl und Healthcheck des neuen Images gelten; der Healthcheck des Gateways bekommt eine Startphase so lang wie die Wartezeit für die Migrationen) und startet ihn.
4. **Migrieren.** Es wartet auf jeden Healthcheck: 2 Minuten für die Konsole, 10 für die Migrationen des Gateways. Ein Absturz, ein Neustart oder ein gestoppter Container schlägt sofort fehl; `unhealthy` lässt die Konsole sofort scheitern, das Gateway aber nicht, das es während einer langen Migration melden kann und erst scheitert, wenn die 10 Minuten um sind.
5. **Wieder erreichbar.** Es liest `compose.yaml` erneut und schreibt die neuen Versionen, wenn die Datei noch die geprüfte ist (oder schon geschrieben); sonst lässt es sie stehen und sagt das. Es taggt die Images, entfernt die vorigen Container und löscht die Markierung.

Ein Fehler vor der Umstellung ändert nichts (`failed`). Ein Fehler danach startet die vorigen Container in umgekehrter Reihenfolge wieder und wartet auf sie (`rolled_back`, oder `rollback_failed` mit dem Grund); ein Container, der angelegt wurde, bevor der Updater ihn festhalten konnte, wird an seinem Namen erkannt und entfernt. Ist das Gateway nach seinem Start gescheitert und gilt das Update als eines, das die Datenbank migriert, wird nur die Konsole wiederhergestellt: Das neue Gateway bleibt, das vorige bleibt gestoppt als `…-previous`, und das Update endet als `needs_attention`. Lässt sich `compose.yaml` nicht schreiben oder hat sie sich inzwischen geändert, endet das Update trotzdem als `succeeded`, mit `compose_unwritable` oder `compose_changed` als Grund. Beim Start wartet der Updater bis zu zwei Minuten auf Docker; dann schließt er ein Update ab, das `online` war (stellt eine halb geschriebene `compose.yaml` aus seiner Kopie wieder her und taggt die Images aus dem gespeicherten Manifest), schließt eines ab, dessen neues Gateway gesund wird, und setzt jedes andere zurück; antwortet Docker nicht, endet das Update als `needs_attention`, oder als `failed`, wenn noch nichts umgestellt war.

Der Updater erreicht Docker nur unter `http://docker-proxy:2375/v1.44/`, dessen Freigabeliste in `compose.yaml` jeden Pfad nennt, den er nutzt. Der Proxy filtert nur nach Methode und Pfad: Den Inhalt von `containers/create` bestimmt der Updater, deshalb ist der Updater gleichbedeutend mit root auf dem Host und optional.

## Die Edge

Caddy liest die Wartungsmarkierung aus dem Volume `maintenance`. Solange sie existiert, bekommt jede Anfrage sofort die Wartungsantwort; sonst tut es ein `502`, `503` oder `504` von Gateway oder Konsole. `/kaeso-maintenance.json` liefert die Markierung, `{"state":"updating","step":"migrating"}`, damit die Seite den Schritt nennen kann, ohne Versionen zu nennen. Die Seite steht inline in `compose.yaml`, mit einer Content-Security-Policy, die genau ihren eigenen Stil und ihr eigenes Skript per Hash erlaubt. `crates/kaeso-server/tests/edge.rs` prüft, dass die JSON-Antworten dem entsprechen, was `ApiError` und `kaeso_codecs::dispatch::encode_error` für dieselbe Meldung erzeugen, dass die Hashes stimmen und dass die Seite nichts von anderswo lädt.

## Tests

| Wo | Was |
| --- | --- |
| `crates/kaeso-update` | Versions-Tags, Kanäle, der Plan mit Hauptversionen, Schritten von Hand und Vorabversionen (mit einem Property-Test), das Manifest mit seinen Migrationen und `manual_since`, das Protokoll |
| `crates/kaeso-server/tests/updates.rs` | Gegen ein Mock-GitHub und einen Mock-Updater: bedingte Anfragen, Fehler und ihre Wartezeiten, die Wartebitte von GitHub für Prüfungen von Hand, Manifeste von anderswo, zwölf Manifeste pro Prüfung, Installationen ohne Internet, Rechte, das Audit-Log, die Sperre auf ein Update, abgelehnte und unbestätigte Starts, die Schonfrist, Fortschritt und jedes Ende |
| `crates/kaeso-updater/tests/verify.rs` | Die echten Attestierungen von Kaeso 0.1.0 bestehen die Prüfung; ein anderes Image, ein anderer Commit, Workflow, eine andere Signatur oder ein anderer Logeintrag nicht, ebenso wenig ein Schlüssel oder eine Stelle außerhalb ihrer Gültigkeit oder ein Bundle ohne signierten Zeitstempel |
| `crates/kaeso-updater/tests/engine.rs`, `server.rs` | Gegen eine nachgebaute Docker-API: ein vollständiges Update, Prüfen vor jedem Download, jede Ablehnung, eine lange Migration, Zurücksetzen, ein Gateway, das einer Person überlassen wird, ein fehlgeschlagenes Zurücksetzen, eine am Ende geänderte oder nicht schreibbare `compose.yaml`, Neustarts in jedem Schritt, Docker, das spät oder gar nicht zurückkommt, die Zustandsdatei, das Token und die API |
| `scripts/release-manifest.test.mjs` | Das Manifest, `manual_since`, die letzten Migrationen und der Commit mit und ohne Release-Commit darüber |
| `e2e/install` | Die Installation mit dem Profil `updater`: Das Gateway erreicht den Updater, der Updater erreicht Docker nur über den Proxy, und die Antworten von Caddy in jedem Format, während Kaeso weg ist, über die offiziellen SDKs; die Jobs, Berechtigungen und die Attestierungsprüfung des Release-Workflows |
| `apps/console` | Die Seite Updates in jedem Zustand, in Unit-Tests und gegen die Mock-Releases und den Mock-Updater des Stacks; die Update-Seite in drei Sprachen, hell und dunkel, mit axe |
