---
title: Performance
description: Die Bundle-Budgets von Konsole, Dokumentation und UI-Bibliothek, die Benchmarks der Codecs und die Ergebnisse des Fuzzings.
order: 8
---

Performance wird gemessen, nicht geschätzt. Die Builds von Konsole und Dokumentation schlagen fehl, wenn eine Seite über ihr Budget wächst, der Build der UI-Bibliothek, wenn eine Komponente es tut, und die Codecs von KPP haben Benchmarks und Fuzz-Targets. Eine Änderung, die ein größeres Budget braucht, hebt es im selben Pull Request an, begründet das und aktualisiert diese Seite.

Alle Größen sind gzip-komprimiert; 1 kB sind 1.024 Bytes.

## Die Konsole

`pnpm --filter kaeso-console build` endet mit `apps/console/scripts/budget.mjs`. Das Skript liest das Manifest des Builds (`dist/.vite/manifest.json`) und addiert das JavaScript, das eine Route vor dem ersten Zeichnen braucht:

- den Entry-Chunk und alles, was er statisch importiert;
- die Lazy-Chunks der Route und jeder Layout-Route darüber (`__root`, `_app`, `orgs/$org`, `projects/$project` und so weiter) mit ihren statischen Importen;
- für die Projektübersicht die Routen-Gates (`src/auth/route-gates.ts`), die der Rahmen neben der Sitzung lädt, bevor die Seite zeichnet.

Code, der erst bei Bedarf lädt, zählt nicht: die Positionierung von Tooltips, der Kalender der Datumsauswahl, die Befehlspalette, der Dialog für die erneute Anmeldung, die Liste der Tastenkürzel, **Organisation erstellen**, der Toaster und die Teile einer Seite, die neben ihren Daten laden. Das Skript lässt den Build außerdem fehlschlagen, wenn ein Modul unter `src/features` im Entry oder in dessen statischen Importen landet, denn jede Seite, die Anmeldung eingeschlossen, lädt zuerst den Entry.

| Route | Datei | Ziel | Obergrenze | Gemessen |
| --- | --- | --: | --: | --: |
| Anmeldung | `src/routes/sign-in.tsx` | 120 kB | 126 kB | 123,1 kB |
| Projektübersicht | `src/routes/_app/orgs/$org/projects/$project/index.tsx` | 180 kB | 190 kB | 181,6 kB |

Eine Route über ihrer Obergrenze lässt den Build fehlschlagen. Das Ziel ist die Größe, die eine Route erreichen soll; noch erreicht es keine der beiden. Die Obergrenze ist die gemessene Größe plus einige Kilobyte, etwa 5 %, wo eine Route geschrumpft ist, sodass eine kleine Änderung hineinpasst und ein Rückschritt fehlschlägt. Eine Obergrenze wird nur mit Begründung angehoben, im Pull Request, der sie braucht, und gesenkt, wenn eine Route kleiner wird.

Von den 123,1 kB der Anmeldung:

| Chunk | Enthält | Größe |
| --- | --- | --: |
| `framework` | React und React DOM mit dem Scheduler, TanStack Router und Query, openapi-fetch sowie API-Client, Sitzung und Formatierung der Konsole | 95,3 kB |
| `ui` | `cn` der Bibliothek mit eigener `tailwind-merge`-Konfiguration, ihr Button, Text, Feld, Eingabefeld und Wortmarke, der Seitentitel und das Layout der öffentlichen Seiten | 13,3 kB |
| Der Entry | Der Routenbaum und seine Preload-Listen | 9,7 kB |
| Die Anmeldeseite | Ihre eigenen Chunks | Der Rest |

Die Projektübersicht fügt den Rahmen der Konsole, die Kennzahlen und Diagramme und die Seite hinzu. Der Rahmen ist die Seitenleiste mit ihren Teilen aus Base UI und Floating UI; ihre Menüs laden, sobald die Seite untätig ist oder beim ersten Hovern, Fokussieren oder Drücken, und das Navigations-Sheet mit dem Dialog von Base UI, sobald die Seite untätig ist oder beim ersten Tippen. Die Diagramme nutzen d3 mit der eigenen linearen Skala der Bibliothek, ohne die Farb-, Interpolations- und Zahlenformat-Module von d3-scale.

`src/main.tsx` fordert die Chunks der Routen, auf die die Adresse passt (`loadPageChunks` in `src/router.tsx`), an, bevor es eine Sprache lädt und rendert; so wird der Code einer Seite geladen, während der Entry noch läuft. Deutsch und Chinesisch holen die Texte der Konsole und der Bibliothek gleichzeitig.

### Was die Zahlen bewegt

- **Routen.** Der Routenbaum steckt im Entry, deshalb sind Pfad, Suchvalidierung und `beforeLoad` jeder Route Teil jeder Seite. Eine Routendatei importiert von ihrer Seite nur Typen; ein Wert-Import zöge das Modul der Seite samt Importen in den Entry. Die Suchprüfung ist eine Funktion in der Routendatei, die `validateSearch` und `Route.useSearch({ select })` der Komponente teilen, sodass sie nur einmal im Entry steht. Loader, Komponenten sowie Pending-, Fehler- und Not-found-Komponenten werden in die eigenen Chunks der Route ausgelagert.
- **Module, die der Entry importiert**, sind vollständig im ersten Zeichnen, mit allem, was sie importieren, auch mit Exporten, die erst spätere Seiten nutzen. Deshalb sind die Abfragen der Sitzung (`src/auth/session-queries.ts`) und die Suchprüfung des Zeitraums (`src/format/period-search.ts`) eigene Module: Die Observer von TanStack Query und die Datumsbeschriftungen bleiben bei den Seiten, die sie nutzen.
- **Preload-Listen.** Für jeden Lazy-Chunk, den der Routenbaum laden kann, trägt der Entry die Liste der nötigen Chunks, damit der Browser sie sofort holt. Jeder Chunk in einer Liste kostet seinen Dateinamen; ein neuer gemeinsamer Chunk, den viele Seiten erreichen, macht also jede Seite größer.
- **Gemeinsame Chunks.** Der Bundler legt Module, die mehrere Lazy-Seiten teilen, in einen Chunk. Teilt eine Seite, die die Übersicht nicht braucht, ein Modul mit einer, die sie braucht, lädt die Übersicht diesen Chunk mit, samt allem, was sonst darin steckt.
- **Die Bibliothek.** Eine Komponente aus `@logicplanes/kaeso-ui` zählt, wenn das erste Zeichnen sie darstellt oder über etwas Dargestelltes importiert; eine, die bei Interaktion lädt, zählt nicht.
- **Erstes Zeichnen oder später.** Was erst nach den Daten, nach einer Interaktion oder nach dem ersten Zeichnen erscheint, kann dann laden. Beispiele: Aufschlüsselung und Seitenbereich der Übersicht (`src/features/project/overview-parts.tsx`), ihre Balkendiagramme (`src/kaeso/usage-bars.tsx`) und Zeitraumauswahl (`src/kaeso/period-picker.tsx`), die Schritte der Anmeldung nach der Adresse (`src/features/sign-in/sign-in-steps.tsx`), die Seitenleiste der Einstellungen, die Menüs der Seitenleiste, der Verbindungs-Toast und die Einzeltasten-Kürzel (`src/shell/listeners.tsx`; Tasten, die vor dem Laden gedrückt werden, hält `src/shell/early-keys.ts` fest).

### Was der Build tut, um sie klein zu halten

In `apps/console/vite.config.ts`:

| Einstellung | Wirkung |
| --- | --- |
| `uiEntryImports` | Schreibt `import { … } from "@logicplanes/kaeso-ui"` auf die Einstiegspunkte der einzelnen Komponenten um und jedes `icons.name` auf den Import genau dieses Icons aus `@hugeicons/core-free-icons`, sodass eine Route nur die Komponenten und Icons enthält, die sie darstellt |
| `codeSplittingOptions` | Lässt das Router-Plugin Loader, Komponente sowie Fehler-, Not-found- und Pending-Komponente jeder Route in eigene Chunks auslagern (`autoCodeSplitting`) |
| `codeSplitting.groups` | Legt das Framework und die Module der Konsole für das erste Zeichnen (`src/api`, `src/auth`, `src/format`) in einen Chunk `framework`, die Teile der Bibliothek, mit denen jede Seite zeichnet (`cn`, Button, Logo, Text, Abschnitt, Feld, Eingabegruppe, Seitentitel, Layout der öffentlichen Seiten und die drei Icons jeder Seite), in einen Chunk `ui` und die Routen-Gates mit dem, was der Rahmen ohnehin importiert, in einen Chunk `gates`. Nach den Seiten aufgeteilt, die sie teilen, ließen sich diese Module schlechter komprimieren und verlängerten jede Preload-Liste |
| `routePreloads`, `resolvePreloads` (`build.modulePreload.resolveDependencies`) | Lassen in den Preload-Listen die Chunks weg, die der Entry schon importiert, unter `_app` die Chunks, die der Rahmen mitbringt, und die Chunks, die nur die Instanz-Einstellungen und der Playground brauchen; diese laden mit dem eigenen Chunk ihrer Seite |
| `chunkFileNames` | Benennt gemeinsame Chunks nur mit sieben Zeichen Hash; Seiten und die Gruppen `framework` und `ui` behalten ihre Namen. Manifest und Source Maps sagen, was jeder enthält |
| `build.modulePreload.polyfill` | Aus: Jeder Browser, auf den der Build zielt, lädt Module selbst vor |
| `resolve.alias` | Leitet den Shim von `use-sync-external-store` auf Reacts eigenes `useSyncExternalStore` um, sodass der Store des Routers und Base UI keine zweite Implementierung mitbringen |
| `themeInit`, `fontPreload` | Liefern das Theme-Skript der Bibliothek, das vor dem Zeichnen läuft, als `/theme-init.js` aus und laden die Oberflächenschrift vor, sodass das erste Zeichnen Theme und Schrift ohne Inline-Skript hat |

## Die Dokumentations-Website

`pnpm --filter kaeso-docs build` endet mit `apps/docs/scripts/budget.ts`, das misst, was eine Seite vor dem ersten Zeichnen braucht, und den Build über einer Obergrenze fehlschlagen lässt.

| Budget | Zählt | Gemessen | Obergrenze |
| --- | --- | --: | --: |
| Der Rahmen, pro Sprache | Den Entry mit seinen statischen Importen und dem Stylesheet, das Website-Modul der Sprache und die Texte der Bibliothek | 201,9 kB (Englisch), 207,4 kB (Deutsch), 207,9 kB (Chinesisch) | 216 kB |
| Der Inhalt einer Seite | Den eigenen Chunk jeder Seite: ihre Blöcke und ihr Inhaltsverzeichnis | höchstens 17,8 kB (`zh-CN/development/kpp/anthropic-messages`) | 24 kB pro Seite |

Der Rahmen wächst mit jeder Seite ein wenig, denn das Website-Modul der Sprache trägt die Seitenleiste mit Titel und Beschreibung jeder Seite, damit die Seitenleiste mit dem Rahmen zeichnet und nie springt. Deutsch und Chinesisch laden zusätzlich die Texte der Bibliothek, etwa 5 kB. Die Obergrenze des Rahmens ist der größte Rahmen mit etwas Spielraum. Der Inhalt einer Seite hat eine eigene Obergrenze, sodass eine zu lang gewordene Seite mit ihrer Adresse fehlschlägt, egal wer sie bearbeitet; die Lösung ist, sie in zwei Seiten aufzuteilen.

Die Suche mit ihrem Index, der Toaster, das Navigations-Sheet und die Grammatiken der Syntaxhervorhebung laden nach dem ersten Zeichnen und zählen nicht. Der Build gibt die Größe der Suche aus, 6,7 kB ohne Index, den Dialog von Base UI eingeschlossen.

| Teil des Rahmens | Warum er da ist |
| --- | --- |
| React und TanStack Router (`framework`) | Jede Seite |
| `AppFrame`, `Sidebar` | Der Rahmen; `Sidebar` bringt das Menü von Base UI mit, und das Navigations-Sheet lädt, sobald die Seite untätig ist oder beim ersten Tippen |
| `Select` | Die Auswahl für Sprache und Theme am Fuß der Seitenleiste |
| Tooltips, `IconButton` | Die Bedienelemente für Suche und Einklappen |
| Die Seite und ihr Renderer | `Text`, `Link`, `Code`, `CodeBlock` (seine Hervorhebung lädt nur mit einem Codeblock), `Banner`, `Table`, `ScrollArea`, `CheckboxBox`, `Separator`, `TableOfContents` sowie `Badge` mit dem Datumsformatierer für das Changelog |
| Das Modul `site` der Sprache | Die Seitenleiste sowie Titel und Beschreibung jeder Seite |

Die Website hält das erste Zeichnen außerdem klein und stabil:

- Importe aus `@logicplanes/kaeso-ui` werden beim Build auf die Einstiegspunkte der einzelnen Komponenten umgeschrieben (`uiEntryImports` in `apps/docs/vite.config.ts`).
- Seiten werden beim Build kompiliert, sodass marked und der Content-Build nie den Browser erreichen.
- Das HTML jeder Seite lädt die Chunks ihrer Sprache und der Seite selbst vor, sodass das erste Zeichnen nicht auf einen zweiten Roundtrip wartet.
- Die Oberflächenschrift wird vorgeladen (`fontPreload`), sodass das erste Zeichnen in Inter erfolgt und nichts umbricht, wenn sie eintrifft.
- Bilder tragen Breite und Höhe aus der Datei, sodass die Seite beim Laden nicht springt.

## Die UI-Bibliothek

`pnpm build:packages` baut `@logicplanes/kaeso-ui` und endet mit `packages/ui/scripts/budgets.mjs`, das das gebaute Paket in `packages/ui/dist` misst und den Build über einem Budget fehlschlagen lässt. Eine App zahlt nur für die Komponenten, die sie importiert, deshalb erfassen die Budgets, was eine Komponente für sich kostet.

| Budget | Zählt | Gemessen | Budget |
| --- | --- | --: | --: |
| Das Fundament | Die gemeinsamen Chunks, die mindestens 40 % der Komponenten-Einstiegspunkte importieren | 5,3 kB | 8 kB |
| `Button` | `components/button.js` und was es importiert, ohne Fundament und Einstiegspunkte anderer Komponenten | 3,0 kB | 3 kB |
| `Menu` | `components/menu.js`, ebenso gezählt | 4,8 kB | 9 kB |
| `DataTable` | `components/data-table.js`, ebenso gezählt | 10,3 kB | 22 kB |
| `styles.css` | Das ganze Stylesheet | 18,0 kB | 28 kB |

Dasselbe Skript lässt den Build fehlschlagen, wenn

- ein Modul in `dist` auf Modulebene eine Anweisung enthält, die ein Bundler behalten muss, sodass ungenutzte Komponenten nicht entfernt werden könnten;
- eine Datei `process.env.NODE_ENV` mit `typeof process` absichert, was Entwicklungsprüfungen in Browser-Bundles belässt;
- `formatMessage`, `useUiText` oder `useLocale` die Formatierer (`format.js`) mitbrächten. `formatMessage` allein sind 0,7 kB.

## KPP

```bash
cargo bench -p kaeso-codecs --bench codecs
```

Jeder Benchmark durchläuft einen vollständigen Zyklus im selben Dialekt: die Wire-Bytes nach KPP dekodieren, dann KPP wieder in Wire-Bytes kodieren. Die Eingaben werden für jeden Dialekt aus demselben KPP-Inhalt gebaut, sodass die Zahlen vergleichbar sind. Gemessen mit criterion (`--warm-up-time 2 --measurement-time 5`) auf einem Apple M5 Pro, Rust 1.99.0, Release-Profil:

| Benchmark | Anthropic Messages | OpenAI Chat | OpenAI Responses |
| --- | --: | --: | --: |
| Konversation mit 100 Nachrichten (ein System-Prompt, ein Tool, 25 Runden aus Benutzertext, Assistententext mit Tool-Aufruf, Tool-Ergebnis und Assistententext), dekodieren und kodieren | 352 µs (78 MiB/s) | 242 µs (105 MiB/s) | 279 µs (90 MiB/s) |
| Text-Stream mit 2.000 Events, in Chunks zu 512 Bytes eingespeist, dekodieren und kodieren | 3,9 ms (etwa 2,0 µs pro Event, 61 MiB/s) | 10,6 ms (etwa 5,3 µs pro Event, 36 MiB/s) | 8,6 ms (etwa 4,3 µs pro Event, 52 MiB/s) |

Für das Gateway kostet eine lange Konversation in jeder Richtung deutlich weniger als eine Millisekunde und ein gestreamtes Token wenige Mikrosekunden, weit unter den zig Millisekunden zwischen den Tokens jedes Modells. Die Stream-Codecs für Chat und Responses leisten pro Event mehr als der von Anthropic: Jeder Chat-Chunk wiederholt den Antwort-Envelope, und jedes Responses-Event trägt eine Sequenznummer, manche wiederholen auch das bisher aufgebaute Item. Dort würde eine Optimierung ansetzen.

CI prüft, dass die Benchmarks bauen (`cargo bench -p kaeso-codecs --bench codecs --no-run`). Eine Änderung an performancekritischem Codec-Code vergleicht einen vollständigen Lauf mit der Tabelle oben.

## Fuzzing

Die [Fuzz-Targets](testing.md#fuzzing) liefen zuletzt je 10 Minuten, alle fünf parallel auf der Maschine oben, mit `-rss_limit_mb=2048 -max_len=65536`:

| Target | Ausführungen | Abdeckung (Kanten) | Spitzenspeicher | Ergebnis |
| --- | --: | --: | --: | --- |
| `sse` | 7.628.740 | 372 | 707 MB | Kein Absturz |
| `decode_anthropic_messages` | 3.599.755 | 11.047 | 588 MB | Kein Absturz |
| `decode_openai_chat` | 2.044.761 | 11.347 | 570 MB | Kein Absturz |
| `decode_openai_responses` | 1.166.478 | 10.263 | 571 MB | Kein Absturz |
| `decode_openai_embeddings` | 11.245.268 | 4.359 | 668 MB | Kein Absturz |

Der Spitzenspeicher ist der des ganzen libFuzzer-Prozesses einschließlich seines Korpus im Speicher; er blieb nach den ersten Minuten konstant, und keine Eingabe erreichte die Grenze von 2 GB. Der Speicher der Bibliothek pro Eingabe ist durch die Größe der Eingabe und die Grenze für SSE-Frames begrenzt (`sse::DEFAULT_MAX_FRAME_BYTES`, 16 MiB).
