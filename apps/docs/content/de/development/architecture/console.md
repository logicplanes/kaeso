---
title: Konsole
description: Wie die Web-Konsole Daten liest, Rechte prüft, ihren Code lädt und Fehler zeigt.
order: 2
---

Die Konsole ist eine statische React-App in `apps/console`, ausgeliefert unter der Domain des Gateways. Ihr Sitzungscookie ist deshalb ein First-Party-Cookie, und sie braucht kein CORS; nur Web-Apps, die die Modell-API aus einem Browser aufrufen, brauchen ihre Origins in `KAESO_CORS_ALLOWED_ORIGINS`. Die Konsole hält nichts außer dem HttpOnly-Cookie, das das Gateway setzt. Listen, Filter, Zeiträume und geöffnete Details stehen in der Adresse, sodass sich Links teilen lassen und die Zurück-Taste funktioniert.

| Teil | Wo |
| --- | --- |
| Routen, Loader und Seiten | `src/routes`, `src/features` |
| Der Client der Platform API, typisiert aus `openapi.json`, und seine Abfragen | `src/api` |
| Sitzung, Rechte und Routen-Guards | `src/auth` |
| Der App-Rahmen, die Seitenleiste, die Befehlspalette und die Tastenkürzel | `src/shell` |
| Zusammenstellungen aus Bibliotheksteilen, die nur die Konsole braucht | `src/kaeso` |
| Die Sprachen und ihre Kataloge | `src/i18n` |
| Geldbeträge, Zeiträume, Beschriftungen und andere Werte, wie die Konsole sie schreibt | `src/format` |
| Generatoren, die Stacks, Beispieldaten, die Übersetzungsprüfung, Budgets und Lizenzen | `scripts` |
| Die Lint-Regeln für reine Layout-Klassen und übersetzten Text | `eslint` |
| Die End-to-End-Suite | `e2e` |
| Mock-Anbieter, Identitätsanbieter, Releases und Hauptschlüssel des Entwicklungs-Stacks | `stack` |

## Regeln

- **Nur die Platform API**, und jeder Aufruf über den Client in `src/api`. Was die API nicht kann, wird nicht gezeigt, und nichts wird vorgetäuscht.
- **Rechte kommen von der API, nie geraten.** Organisationsrechte kommen aus `auth/session`, Projektrechte aus `GET …/projects/{project}/my-rights` und Name, Beschreibung, Kategorie und Voraussetzungen jedes Rechts aus `GET orgs/{org}/rights`. Ein `403` ist endgültig: Ein Bereich, den jemand nicht sehen darf, ist ausgeblendet, und eine einzelne Aktion ist deaktiviert, mit dem Namen des Rechts und dem Hinweis, wer es vergeben kann (`useNeedsRight`).
- **Auch Anbieter kommen von der API.** Name, Schlüsselseite, Schlüsselpräfix und Formate jedes Anbieters kommen aus `GET orgs/{org}/providers`, das die Route der Organisation einmal lädt und eine Stunde lang behält. Die Konsole nennt keinen Anbieter in ihrem Code, also erscheint ein Anbieter, den das Gateway hinzufügt, ohne neue Konsole. Eine ID, die das Gateway nicht mehr listet, etwa in alten Protokollen, erscheint als die ID mit großem Anfangsbuchstaben.
- **Keine Geheimnisse im Browser.** Die Konsole liest das Sitzungscookie nie, und sie zeigt Geheimnisse von Schlüsseln einmal und speichert sie nie. `localStorage` enthält nur die letzte Organisation und das letzte Projekt, zuletzt geöffnete Einträge, das Theme, die Sprache, Breite und Zustand der Seitenleiste, die Spalten der Anfrageprotokolle und die noch nicht gesendeten Prompts des Playgrounds. `sessionStorage` enthält das Token einer Einladung, bis sie angenommen ist.
- **Dialoge öffnen frisch.** Was ein Dialog hält, etwa Felder, den Schritt, ein Ergebnis oder ein einmal gezeigtes Geheimnis, lebt in einer Komponente, die bei jedem Öffnen neu eingehängt wird: in einem `DialogScope` oder nur gerendert, solange der Dialog offen ist. Ein Sheet für eine Zeile zeigt nie die Daten einer anderen.
- **Tokens in Links stehen im Fragment.** Die Konsole liest `location.hash` einmal, ruft die API auf und ersetzt die Adresse durch eine ohne Fragment.
- **Jedes sichtbare Element stammt aus `@logicplanes/kaeso-ui`**, und `className` enthält nur Layout: Margin, Breite, Grid, Flex, Gap und Position. `pnpm lint` setzt das mit den Regeln in `apps/console/eslint/kaeso.js` durch.
- **Jedes Wort in drei Sprachen**, über `t()` ([Übersetzungen](../translations.md#die-konsole)).
- **Beschreibungen sind ein Satz, der auf eine Zeile passt.** Die Grenze hängt vom Ort ab: 60 Zeichen für eine Einstellungszeile (50 neben einem Eingabefeld), 80 für einen Abschnitt oder eine Seite (70 neben Aktionen), 100 für einen Dialog, 90 für einen leeren Zustand und 60 für einen Toast. `src/copy.test.ts` prüft Zeilen, Abschnitte und Seiten in jeder Sprache, Deutsch und Chinesisch mit 15 % Spielraum.

## Daten

Jedes Lesen ist eine TanStack-Query-Abfrage mit der Anfrage als Schlüssel, und jede Änderung ist eine Mutation, die alles invalidiert, was sie berührt, dazu das Audit-Log.

- **Der Client** in `src/api/client.ts` ist `openapi-fetch` mit den Typen, die `pnpm --filter kaeso-console generate` aus `openapi.json` nach `src/api/generated` schreibt. Die Skripte `dev`, `build`, `typecheck`, `test` und `e2e` der Konsole führen den Generator zuerst aus, und seine Ausgabe wird nie committet. Der Client sendet das Sitzungscookie und macht aus jedem Fehlschlag einen `ApiError` mit `type`, `message` und `param` der API sowie der `kaeso-request-id` (`src/api/errors.ts`).
- **Listen** blättern mit Cursorn (`src/api/paging.ts`), und **Exporte** sind gestreamte Downloads (`src/api/download.ts`).
- **Suchparameter.** Jede Route prüft ihre Suchparameter in einer Funktion, die `validateSearch` und `Route.useSearch({ select })` der Seite gemeinsam nutzen. Ein Wert, den eine Seite nicht kennt, fällt auf ihren Standard zurück.
- **Aktualität** (`src/query.ts`). Daten aus dem Cache erscheinen sofort und werden im Hintergrund neu geladen. Abfragen gelten 10 Sekunden lang als aktuell, einige, die sich selten ändern, länger, etwa die Details einer Anfrage fünf Minuten lang. Die Sitzung gilt fünf Sekunden lang als aktuell, sodass die Teile einer Seite sie teilen, und wird beim Fokus und jede Minute neu geprüft. Eine lange Liste lädt beim Fokus nur neu, solange eine Seite von ihr geladen ist; wer zum Fenster zurückkehrt, lädt also nie jede Seite neu, durch die er gescrollt hat. Eine Änderung lädt nur neu, was sie betrifft: Die Organisation selbst hat den Schlüssel `["orgs", org, "view"]`, sodass ihr Neuladen die Listen in der Organisation in Ruhe lässt. Änderungen und Client-Fehler werden nie wiederholt, andere fehlgeschlagene Lesevorgänge zweimal.

## Styling

Die Konsole hat keine eigenen Styles. `@logicplanes/kaeso-ui` liefert jedes sichtbare Element und das Stylesheet, und der Code der Konsole nutzt Tailwind nur für Layout, aus derselben Token-Skala. Ein Vite-Plugin liefert das Theme-Skript der Bibliothek, das vor dem ersten Zeichnen läuft, als `/theme-init.js` aus, sodass die Content Security Policy kein Inline-Skript erlaubt. Die Policy verlangt außerdem Trusted Types für Skripte, sodass kein Code eine Zeichenkette in HTML oder ein Skript verwandeln kann, und der Dev-Server der End-to-End-Tests sendet diese Regel ebenfalls.

## Bundles

Der Router teilt die Routen auf (`autoCodeSplitting`), sodass der Code einer Seite mit der Seite lädt. Der Build schreibt Importe aus `@logicplanes/kaeso-ui` auf die Einstiegspunkte der einzelnen Komponenten um und jedes `icons.name` auf den Import genau dieses Icons (`uiEntryImports` in `vite.config.ts`). Eine Seite bringt dann nur die Komponenten und Icons mit, die sie zeichnet.

Was das erste Zeichnen nicht braucht, lädt bei Gebrauch: der Toaster, die Befehlspalette, der Dialog für eine erneute Anmeldung, die Liste der Tastenkürzel, „Organisation erstellen“, die Menüs der Seitenleiste, Tooltips und der Kalender der Datumsauswahl. Die Menüs der Seitenleiste und der Kalender laden außerdem, sobald die Seite untätig ist, damit sie sich beim ersten Klick sofort öffnen. Aufschlüsselung, Kostenanteil und Zeitraumauswahl der Projekt- und Organisationsübersicht sowie die Anmeldeschritte nach der E-Mail-Adresse laden zusammen mit den Daten, auf die sie warten. Das Anfrageprotokoll, das Audit-Log eines Projekts und die Übersicht eines Projekts starten ihre Daten im Loader der Route, aus einem eigenen Modul (`src/api/queries/route-prefetch.ts`), das nur diese Routen laden.

`pnpm build` schlägt fehl, wenn das erste Zeichnen einer Route ihre Obergrenze in `scripts/budget.mjs` überschreitet oder Seitencode aus `src/features` in den Einstiegspunkt gerät. So kommen Sie wieder darunter:

- Importieren Sie in eine Routendatei nur Typen aus ihrer Seite. Der Routenbaum steckt im Einstiegspunkt, ein Wert-Import zieht die Seite also in jede Seite.
- Geben Sie Code, den der Einstiegspunkt importiert, den aber erst spätere Seiten nutzen, ein eigenes Modul.
- Laden Sie, was das erste Zeichnen nicht braucht, erst bei Gebrauch oder sobald die Seite untätig ist.

## Fehler

Jeder Fehler ist ein Toast; Fehler erscheinen nie im Seiteninhalt.

- Änderungen laufen über `useApiMutation` (`src/auth/mutation.ts`), öffentliche Formulare über `useSubmit` und fehlgeschlagene Ladevorgänge über den Handler des Query-Caches (`src/query.ts`), der „Erneut versuchen“ anbietet.
- Fehler des Gateways tragen die Anfrage-ID.
- Änderungen an Katalogmodellen und Limits senden `If-Match` mit der Revision, auf der sie gemacht wurden. Ein `412` lädt neu, was sich geändert hat, sagt das und schließt den Bearbeitungsdialog.
- Ein fehlendes Objekt (404) ist ein eigener Zustand der Seite oder des Sheets, und eine beendete Sitzung führt zur Anmeldung.
