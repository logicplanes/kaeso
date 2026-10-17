---
title: Konsole
description: Wie die Web-Konsole Daten liest, ihre Seiten zeichnet, ihren Code lädt und Fehler zeigt.
order: 2
---

Die Konsole ist eine statische React-App in `apps/console`, ausgeliefert unter der Domain des Gateways. Sie meldet Menschen über das Gateway an und hält nichts außer dem HttpOnly-Sitzungs-Cookie, das das Gateway setzt. Listen, Filter, Zeiträume und geöffnete Details stehen in der Adresse, sodass sich Links teilen lassen und die Zurück-Taste funktioniert.

| Teil | Ort |
| --- | --- |
| Routen, Loader und Seiten | `src/routes`, `src/features` |
| Der Client der Platform API, typisiert aus `openapi.json`, und seine Abfragen | `src/api` |
| Sitzung, Rechte und Routen-Guards | `src/auth` |
| Der App-Rahmen, die Seitenleiste, die Befehlspalette und die Tastenkürzel | `src/shell` |
| Zusammenstellungen aus Bibliotheksteilen, die nur die Konsole braucht | `src/kaeso` |
| Die Sprachen und ihre Kataloge | `src/i18n` |
| Geldbeträge, Zeiträume, Beschriftungen und andere Werte, wie die Konsole sie schreibt | `src/format` |
| Generatoren für die API-Typen und den Routenbaum, die Stacks, Beispieldaten, die Übersetzungsprüfung, Budgets und Lizenzen | `scripts` |
| Die Lint-Regeln für reine Layout-Klassen und übersetzten Text | `eslint` |
| Die End-to-End-Suite | `e2e` |
| Mock-Anbieter, Identitätsanbieter und Hauptschlüssel des Entwicklungs-Stacks | `stack` |

## Regeln

- **Nur die Platform API**, und jeder Aufruf über den Client in `src/api`. Was die API nicht kann, wird nicht gezeigt, und nichts wird vorgetäuscht.
- **Rechte kommen von der API, nie geraten.** Organisationsrechte kommen aus `auth/session`, Projektrechte aus `GET …/projects/{project}/my-rights`, Name, Beschreibung, Kategorie und Voraussetzungen jedes Rechts aus `GET orgs/{org}/rights`. Ein `403` ist endgültig: Ein Bereich, den jemand nicht sehen darf, ist ausgeblendet, und eine einzelne Aktion ist deaktiviert, mit dem Namen des Rechts und wer es vergeben kann (`useNeedsRight`).
- **Keine Geheimnisse im Browser.** Die Konsole liest das Sitzungs-Cookie nie. Geheimnisse von Schlüsseln werden einmal gezeigt und nie gespeichert. `localStorage` enthält nur die letzte Organisation und das letzte Projekt, zuletzt geöffnete Einträge, das Theme, die Sprache, Breite und Zustand der Seitenleiste, die Spalten der Anfrageprotokolle und die noch nicht gesendeten Prompts des Playgrounds; `sessionStorage` enthält das Token einer Einladung, bis sie angenommen ist.
- **Dialoge öffnen frisch.** Was ein Dialog, ein Sheet oder ein Schrittdialog hält (Felder, Schritt, Ergebnis, ein einmal gezeigtes Geheimnis), lebt in einer Komponente, die bei jedem Öffnen neu eingehängt wird: in einem `DialogScope`, der sie auch wieder aushängt, sobald der Dialog fertig geschlossen ist, oder nur gerendert, solange der Dialog offen ist. Vom letzten Öffnen kommt nichts zurück, und ein Sheet für eine Zeile zeigt nie die Daten einer anderen.
- **Tokens in Links stehen im Fragment.** Die Konsole liest `location.hash` einmal, ruft die API auf und ersetzt die Adresse durch eine ohne Fragment.
- **Jedes sichtbare Element stammt aus `@logicplanes/kaeso-ui`**, und `className` enthält nur Layout: Margin, Breite, Grid, Flex, Gap und Position. `pnpm lint` erzwingt das mit den Regeln in `apps/console/eslint/kaeso.js`.
- **Jedes Wort in drei Sprachen**, über `t()` ([Übersetzungen](../translations.md#die-konsole)).
- **Beschreibungen sind ein Satz, der auf eine Zeile passt.** Die Grenzen hängen vom Ort ab: 60 Zeichen für eine Einstellungszeile (50 neben einem Eingabefeld), 80 für einen Abschnitt oder eine Seite (70 neben Aktionen), 100 für einen Dialog, 90 für einen leeren Zustand und 60 für einen Toast. `src/copy.test.ts` prüft Einstellungszeilen, Abschnitte und Seiten in jeder Sprache, Deutsch und Chinesisch mit 15 % Spielraum.

## Daten

Jedes Lesen ist eine TanStack-Query-Abfrage mit der Anfrage als Schlüssel; jede Änderung ist eine Mutation, die invalidiert, was sie berührt, und das Audit-Log.

- **Der Client** in `src/api/client.ts` ist `openapi-fetch` mit den Typen, die `pnpm --filter kaeso-console generate` aus der `openapi.json` des Repositorys nach `src/api/generated` schreibt. Er sendet das Sitzungs-Cookie (gleicher Origin) und macht aus jedem Fehlschlag einen `ApiError` mit `type`, `message` und `param` der API sowie der `kaeso-request-id` (`src/api/errors.ts`).
- **Listen** blättern mit Cursorn (`src/api/paging.ts`); **Exporte** sind gestreamte Downloads mit dem Cookie (`src/api/download.ts`).
- **Suchparameter.** Filter, Zeiträume und geöffnete Sheets stehen in der Adresse. Jede Route prüft ihre Suchparameter in einer Funktion, die `validateSearch` und `Route.useSearch({ select })` der Seite gemeinsam nutzen, weil der Router einer Seite auch die Rohwerte ihrer Eltern übergibt; ein Wert, den eine Seite nicht kennt, fällt auf ihren Standard zurück.
- **Aktualität** (`src/query.ts`). Daten aus dem Cache erscheinen sofort und werden im Hintergrund neu geladen. Abfragen gelten 10 Sekunden lang als aktuell; einige, die sich selten ändern, länger, etwa ein Projekt und seine Rechte (60 Sekunden) und der Rechtekatalog (30 Minuten). Die Sitzung wird immer neu geprüft. Client-Fehler und Änderungen werden nie wiederholt, andere fehlgeschlagene Lesevorgänge zweimal.

## Styling

Die Konsole hat keine eigenen Styles. `@logicplanes/kaeso-ui` liefert jedes sichtbare Element und das Stylesheet (`src/styles.css` importiert den Tailwind-Einstieg der Bibliothek), und der Code der Konsole nutzt Tailwind nur für Layout, aus derselben Token-Skala. Ein Vite-Plugin liefert das Theme-Skript der Bibliothek, das vor dem ersten Zeichnen läuft, als `/theme-init.js` aus, sodass die Content Security Policy kein Inline-Skript erlaubt.

## Bundles

Der Router teilt die Routen auf (`autoCodeSplitting`), sodass der Code einer Seite mit der Seite lädt. Der Code der Konsole importiert `@logicplanes/kaeso-ui` als ein Paket; der Build schreibt diese Importe auf die Einstiegspunkte der einzelnen Komponenten um und jedes `icons.name` auf den Import genau dieses Icons (`uiEntryImports` in `vite.config.ts`), sodass eine Seite nur die Komponenten und Icons mitbringt, die sie zeichnet. Die Bibliothek kennzeichnet ihre Komponenten-Factories auf Modulebene als pure, sodass eine verwendete Komponente nicht den Rest ihrer Datei mitzieht.

Was das erste Zeichnen nicht braucht, lädt bei Gebrauch: der Toaster, die Befehlspalette, der Dialog für eine erneute Anmeldung, die Liste der Tastenkürzel, „Organisation erstellen“, die Menüs der Seitenleiste, Tooltips und der Kalender der Datumsauswahl. Die Menüs der Seitenleiste und der Kalender laden außerdem, sobald die Seite untätig ist, damit sie sich beim ersten Druck sofort öffnen. Aufschlüsselung und Seitenspalte der Übersicht sowie die Anmeldeschritte nach der E-Mail-Adresse laden zusammen mit den Daten, auf die sie warten. `pnpm build` prüft das Ergebnis gegen die [Budgets](../performance.md#die-konsole).

## Fehler

Jeder Fehler ist ein Toast; nichts zeigt einen Fehler in der Seite.

- Änderungen laufen über `useApiMutation` (`src/auth/mutation.ts`), öffentliche Formulare über `useSubmit` und fehlgeschlagene Ladevorgänge über den Handler des Query-Caches (`src/query.ts`), der „Erneut versuchen“ anbietet.
- Fehler des Gateways tragen die Request-ID.
- Änderungen an Katalogmodellen und Limits senden `If-Match` mit der Revision, auf der sie gemacht wurden. Ein `412` lädt neu, was sich geändert hat, sagt das und schließt den Dialog der Änderung.
- Ein fehlendes Objekt (404) ist ein eigener Zustand der Seite oder des Sheets, und eine beendete Sitzung führt zur Anmeldung.
