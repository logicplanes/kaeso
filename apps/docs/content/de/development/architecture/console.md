---
title: Konsole
description: Wie die Web-Konsole Daten liest, ihre Seiten zeichnet, ihren Code lädt und Fehler zeigt.
order: 2
---

Die Konsole ist eine statische React-App in `apps/console`, ausgeliefert auf derselben Domain wie das Gateway. Sie meldet Personen über das Gateway an und behält nichts außer dem HttpOnly-Sitzungscookie, das das Gateway setzt. Listen, Filter, Zeiträume und geöffnete Details stehen in der Adresse, sodass sich Links teilen lassen und der Zurück-Button funktioniert.

| Teil | Ort |
| --- | --- |
| Routen, Loader und Seiten | `src/routes`, `src/features` |
| Der Client der Platform API, typisiert aus `openapi.json`, und seine Abfragen | `src/api` |
| Sitzung, Rechte und Routen-Guards | `src/auth` |
| Der App-Rahmen, die Seitenleiste, die Befehlspalette und die Tastenkürzel | `src/shell` |
| Kompositionen aus Bibliotheksteilen, die nur die Konsole nutzt | `src/kaeso` |
| Die Sprachen und ihre Kataloge | `src/i18n` |
| Beträge, Zeiträume, Beschriftungen und andere Werte, wie die Konsole sie schreibt | `src/format` |
| Die Generatoren für API-Typen und Routenbaum, die Stacks, Beispieldaten, die Übersetzungsprüfung, Budgets und Lizenzen | `scripts` |
| Die Lint-Regeln für Klassen nur für Layout und übersetzten Text | `eslint` |
| Die End-to-End-Suite | `e2e` |
| Mock-Anbieter, Identitätsanbieter und Master-Key des Entwicklungs-Stacks | `stack` |

## Regeln

- **Nur die Platform API**, und jeder Aufruf über den Client in `src/api`. Was die API nicht kann, wird nicht gezeigt, und nichts wird vorgetäuscht.
- **Rechte kommen von der API, nie geraten.** Rechte in der Organisation aus `auth/session`, Rechte im Projekt aus `GET …/projects/{project}/my-rights` und Name, Beschreibung, Kategorie und Voraussetzungen jedes Rechts aus `GET orgs/{org}/rights`. Ein `403` ist das letzte Wort: Ein Bereich, den eine Person nicht sehen kann, wird ausgeblendet, und eine einzelne Aktion wird deaktiviert, mit dem Namen des Rechts und wer es vergeben kann (`useNeedsRight`).
- **Keine Geheimnisse im Browser.** Die Sitzung ist ein Cookie, das die Konsole nie liest. Schlüsselgeheimnisse werden einmal gezeigt und nie gespeichert. `localStorage` enthält nur die zuletzt gewählte Organisation und das Projekt, zuletzt geöffnete Einträge, das Theme, die Sprache, Breite und Zustand der Seitenleiste, die Spalten der Anfrageprotokolle und die noch nicht gesendeten Prompts des Playgrounds; `sessionStorage` behält das Token einer Einladung, bis sie angenommen ist, und die Adresse, zu der nach einer erneuten Anmeldung zurückgekehrt wird.
- **Tokens in Links stehen im Fragment.** Die Konsole liest `location.hash` einmal, ruft die API auf und ersetzt die Adresse durch eine ohne Fragment.
- **Jedes sichtbare Element stammt aus `@logicplanes/kaeso-ui`**, und `className` enthält nur Layout: Margin, Breite, Grid, Flex, Gap und Position. `pnpm lint` erzwingt das mit den Regeln in `apps/console/eslint/kaeso.js`.
- **Jedes Wort in drei Sprachen**, über `t()` ([Übersetzungen](../translations.md#die-konsole)).
- **Beschreibungen sind ein Satz, der in eine Zeile passt**, mit Grenzen je nach Ort: 60 Zeichen für eine Einstellungszeile (50 neben einem Eingabefeld), 80 für einen Abschnitt oder eine Seite (70 neben Aktionen), 100 für einen Dialog, 90 für einen leeren Zustand, 60 für einen Toast. `src/copy.test.ts` prüft Einstellungszeilen, Abschnitte und Seiten in jeder Sprache, Deutsch und Chinesisch mit 15 % Spielraum.

## Daten

Jeder Lesezugriff ist eine TanStack-Query-Abfrage mit der Anfrage als Schlüssel; jede Änderung ist eine Mutation, die invalidiert, was sie berührt, und das Audit-Log. Der Client in `src/api/client.ts` ist `openapi-fetch` mit den Typen, die `pnpm --filter kaeso-console generate` aus der `openapi.json` des Repositorys nach `src/api/generated` schreibt; er sendet das Sitzungscookie (gleicher Origin) und macht aus jedem Fehlschlag einen `ApiError` mit `type`, `message` und `param` der API und der `kaeso-request-id` (`src/api/errors.ts`).

Listen blättern mit Cursorn (`src/api/paging.ts`); Exporte sind gestreamte Downloads mit dem Cookie (`src/api/download.ts`).

Filter, Zeiträume und geöffnete Sheets stehen in der Adresse. Jede Route prüft ihre Suchparameter in einer Funktion, die sich `validateSearch` und `Route.useSearch({ select })` der Seite teilen, weil der Router einer Seite auch die Rohwerte ihrer übergeordneten Routen übergibt; ein Wert, den eine Seite nicht kennt, fällt auf ihren Standard zurück.

Standards für Abfragen (`src/query.ts`): Daten aus dem Cache erscheinen sofort und werden im Hintergrund neu geladen; Listen sind 10 Sekunden lang frisch, der Katalog und Einstellungen 60, und die Sitzung wird immer neu validiert; Client-Fehler und Änderungen werden nie wiederholt, andere fehlgeschlagene Lesezugriffe zweimal.

## Styling

Die Konsole hat kein eigenes. `@logicplanes/kaeso-ui` liefert jedes sichtbare Element und das Stylesheet (`src/styles.css` importiert den Tailwind-Einstieg der Bibliothek). Der Code der Konsole nutzt Tailwind nur für Layout, aus derselben Token-Skala. Das Theme-Skript der Bibliothek, das vor dem ersten Rendern läuft, wird von einem Vite-Plugin als `/theme-init.js` ausgeliefert, sodass die Content Security Policy kein Inline-Skript zulässt.

## Bundles

Der Router teilt die Routen auf (`autoCodeSplitting`), sodass der eigene Code einer Seite mit der Seite lädt. Der Code der Konsole importiert aus `@logicplanes/kaeso-ui` als ein Paket, und der Build schreibt diese Importe auf die Einstiegspunkte der einzelnen Komponenten der Bibliothek um und jedes `icons.name` auf den Import genau dieses Icons (`uiEntryImports` in `vite.config.ts`), sodass eine Seite nur die Komponenten und Icons enthält, die sie zeichnet. Die Bibliothek markiert ihre Komponenten-Factories auf Modulebene als pure, sodass eine genutzte Komponente nicht den Rest ihrer Datei mitbringt.

Teile, die das erste Rendern nicht braucht, laden bei Bedarf: der Toaster, die Befehlspalette, der Dialog für eine erneute Anmeldung, die Liste der Tastenkürzel, „Organisation erstellen“, Tooltips und der Kalender der Datumsauswahl. Aufschlüsselung und Seitenbereich der Übersicht sowie die Schritte der Anmeldung nach der Adresse laden zusammen mit den Daten, auf die sie warten. `pnpm build` prüft das Ergebnis gegen die [Budgets](../performance.md#die-konsole).

## Fehler

Jeder Fehler ist ein Toast; nichts zeigt einen Fehler in der Seite. Änderungen laufen über `useApiMutation` (`src/auth/mutation.ts`), öffentliche Formulare über `useSubmit` und fehlgeschlagene Ladevorgänge über den Handler des Query-Caches (`src/query.ts`), der „Erneut versuchen“ anbietet. Fehler des Gateways tragen die Request-ID. Ein fehlendes Objekt (404) ist ein eigener Zustand der Seite oder des Sheets, und eine beendete Sitzung führt zur Anmeldung.
