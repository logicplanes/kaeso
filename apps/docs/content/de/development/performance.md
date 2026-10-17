---
title: Performance
description: Die Bundle-Budgets von Konsole, Dokumentation und UI-Bibliothek, die Benchmarks der Codecs und die Ergebnisse des Fuzzings.
order: 8
---

Performance wird gemessen, nicht geschätzt: Die Builds der Konsole und der Dokumentation schlagen fehl, wenn eine Seite über ihr Budget wächst, der Build der UI-Bibliothek, wenn eine Komponente es tut, und für KPPs Codecs gibt es Benchmarks und Fuzz-Targets. Eine Änderung, die ein größeres Budget braucht, hebt die Obergrenze im selben Pull Request an, aktualisiert diese Seite und nennt den Grund.

## Die Konsole

`pnpm --filter kaeso-console build` endet mit `apps/console/scripts/budget.mjs`, das das Manifest des Builds (`dist/.vite/manifest.json`) liest und, gzip-komprimiert, das JavaScript zusammenzählt, das eine Route vor dem ersten Rendern braucht:

- den Entry-Chunk und alles, was er statisch importiert;
- die Lazy-Chunks der Route und jeder Layout-Route darüber (`__root`, `_app`, `orgs/$org`, `projects/$project` und so weiter), mit allem, was sie statisch importieren;
- für die Projektübersicht die Routenentscheidungen der Konsole (`src/auth/route-gates.ts`), die der Rahmen neben der Sitzung lädt, bevor die Seite rendert.

Dynamische Importe, die bei Bedarf laden, werden nicht mitgezählt: die Positionierung der Tooltips, der Kalender der Datumsauswahl, die Befehlspalette, der Dialog für eine erneute Anmeldung, die Liste der Tastenkürzel, „Organisation erstellen“, der Toaster und die Teile einer Seite, die neben ihren Daten laden. Das Skript lässt den Build außerdem fehlschlagen, wenn ein Modul unter `src/features` im Entry oder in dessen statischen Importen steckt, da jede Seite, die Anmeldung eingeschlossen, zuerst den Entry lädt.

Eine Route lässt den Build fehlschlagen, wenn sie über ihrer Obergrenze liegt. Das Ziel ist die Größe, die eine Route erreichen soll; die Obergrenze ist die gemessene Größe, aufgerundet auf das nächste Kilobyte; sie wird nur mit Begründung angehoben, in dem Pull Request, der es braucht.

| Route | Datei | Ziel | Obergrenze | Gemessen, gzip |
| --- | --- | --: | --: | --: |
| Anmeldung | `src/routes/sign-in.tsx` | 120 kB | 126 kB | 126,0 kB |
| Projektübersicht | `src/routes/_app/orgs/$org/projects/$project/index.tsx` | 180 kB | 206 kB | 205,9 kB |

1 kB sind 1.024 Bytes. Noch erreicht keine der beiden Routen ihr Ziel. Von den 126,0 kB der Anmeldung entfallen 98,1 kB auf den Chunk `framework` (React und React DOM mit dem Scheduler, TanStack Router und Query, openapi-fetch sowie API-Client, Sitzung und Formatierung der Konsole), 14,3 kB auf den Chunk `ui` (`cn` der Bibliothek mit seiner eigenen Einrichtung von `tailwind-merge`, ihr Button, Text, Feld, Eingabefeld und Logo, der Seitentitel und das Layout der öffentlichen Seiten), 9,5 kB auf den Entry mit dem Routenbaum und seinen Preload-Listen und der Rest auf die eigenen Chunks der Anmeldeseite. Die Projektübersicht fügt den Rahmen der Konsole hinzu (die Seitenleiste mit ihren Teilen aus Base UI und Floating UI; ihre Menüs laden, sobald die Seite im Leerlauf ist oder beim ersten Hover, Fokus oder Drücken, ebenso wie die Datumsauswahlen der Bibliothek), die Kennzahlenkacheln und Diagramme mit ihren d3-Modulen und die Seite selbst.

### Was die Zahlen bewegt

- **Routen.** Der Routenbaum steckt im Entry, daher fügt jede Route ihren Pfad, ihre Prüfung der Suchparameter und ihr `beforeLoad` jeder Seite hinzu. Eine Routendatei importiert aus ihrer Seite nur Typen: Ein Wert brächte das Modul der Seite und dessen Importe in den Entry. Ihre Prüfung der Suchparameter ist eine Funktion in der Routendatei, die sich `validateSearch` und `Route.useSearch({ select })` der Komponente teilen, sodass die Prüfung einmal im Entry steckt und die abgespaltene Komponente sie von dort importiert. Loader, Komponenten sowie Pending-, Error- und Not-found-Komponenten werden in eigene Chunks der Route ausgelagert.
- **Preload-Listen.** Für jeden Lazy-Chunk, den der Routenbaum laden kann, trägt der Entry die Liste der Chunks, die er braucht, sodass der Browser sie sofort abruft. Jeder Chunk in einer Liste kostet seinen Dateinamen; ein neuer gemeinsamer Chunk, den viele Seiten erreichen, macht jede Seite größer.
- **Gemeinsame Chunks.** Der Bundler legt Module, die sich mehrere Lazy-Seiten teilen, in einen gemeinsamen Chunk. Teilt eine Seite, die die Übersicht nicht braucht, ein Modul mit einer, die sie braucht, lädt die Übersicht diesen Chunk ebenfalls, mit allem, was der Bundler sonst noch hineingelegt hat.
- **Die Bibliothek.** Eine Komponente aus `@logicplanes/kaeso-ui`, die ein erstes Rendern zeichnet oder die eine dort gezeichnete Komponente importiert, gehört zum Budget; eine, die erst bei einer Interaktion lädt, nicht.
- **Erstes Rendern oder später.** Alles, was eine Seite erst nach dem Eintreffen ihrer Daten, nach einer Interaktion oder nach dem ersten Rendern zeigt, kann dann laden: Aufschlüsselung und Seitenbereich der Übersicht (`src/features/project/overview-parts.tsx`), ihre Balkendiagramme (`src/kaeso/usage-bars.tsx`) und ihre Zeitraumauswahl (`src/kaeso/period-picker.tsx`), die Schritte der Anmeldung nach der Adresse (`src/features/sign-in/sign-in-steps.tsx`), die Seitenleiste der Einstellungen, die Menüs der Seitenleiste, der Verbindungs-Toast und die Einzeltasten-Kürzel (`src/shell/listeners.tsx`, wobei Tasten, die vor dem Laden gedrückt werden, in `src/shell/early-keys.ts` festgehalten werden).

### Was der Build tut, um sie klein zu halten

`apps/console/vite.config.ts`:

- **`uiEntryImports`** schreibt `import { … } from "@logicplanes/kaeso-ui"` auf die Einstiegspunkte der einzelnen Komponenten der Bibliothek um und jedes `icons.name` auf den Import genau dieses Icons aus `@hugeicons/core-free-icons`, sodass eine Route nur die Komponenten und Icons enthält, die sie zeichnet.
- **`codeSplittingOptions`** im Router-Plugin lagert Loader, Komponente, Error-, Not-found- und Pending-Komponente jeder Route in eigene Chunks aus (`autoCodeSplitting`).
- **`codeSplitting.groups`** legt das Framework und die Module der Konsole für das erste Rendern (`src/api`, `src/auth`, `src/format`) in einen Chunk `framework`, die Teile der Bibliothek, mit denen jede Seite rendert (`cn`, Button, Logo, Text, Abschnitt, Feld, Eingabegruppe, der Seitentitel, das Layout der öffentlichen Seiten und die drei Icons, die jede Seite zeichnet), in einen Chunk `ui` und die Routen-Gates mit dem, was der Rahmen ohnehin importiert, in einen Chunk `gates`. Aufgeteilt danach, welche Seiten sie teilen, ließen sich diese Module schlechter komprimieren und brächten jeder Preload-Liste weitere Dateinamen.
- **`routePreloads` und `resolvePreloads`** (`build.modulePreload.resolveDependencies`) lassen in den Preload-Listen die Chunks weg, die der Entry schon importiert, unter `_app` die Chunks, die der Rahmen der Konsole mitbringt, und die Chunks, die nur die Instanz-Einstellungsseiten und der Playground brauchen; diese laden mit dem eigenen Chunk ihrer Seite.
- **`chunkFileNames`** benennt gemeinsame Chunks allein nach sieben Zeichen ihres Hashs; Seiten und die Gruppen `framework` und `ui` behalten ihre Namen. Das Manifest und die Source Maps sagen, was jeder enthält.
- **`build.modulePreload.polyfill`** ist abgeschaltet: Jeder Browser, auf den der Build zielt, lädt Module selbst vorab.
- **`resolve.alias`** leitet den Shim von `use-sync-external-store` auf Reacts eigenes `useSyncExternalStore` um, sodass der Store des Routers und Base UI keine zweite Implementierung mitbringen.
- **`themeInit`** liefert das Theme-Skript der Bibliothek, das vor dem ersten Rendern läuft, als `/theme-init.js` aus, und **`fontPreload`** lädt die Oberflächenschrift vorab, die das Stylesheet verwendet, sodass das erste Rendern ohne Inline-Skript schon sein Theme und seine Schrift hat.

## Die Dokumentations-Website

`pnpm --filter kaeso-docs build` endet mit `apps/docs/scripts/budget.ts`, das misst, was eine Seite gzip-komprimiert vor dem ersten Rendern braucht, und den Build fehlschlagen lässt, wenn eine Obergrenze überschritten ist.

| Budget | Was gezählt wird | Gemessen | Obergrenze |
| --- | --- | --: | --: |
| Der Rahmen in jeder Sprache | Der Entry mit seinen statischen Importen und dem Stylesheet, das Website-Modul der Sprache und die Texte der Bibliothek | 204,6 kB (Englisch), 210,1 kB (Deutsch), 210,6 kB (Chinesisch) | 216 kB |
| Der Inhalt einer Seite | Der eigene Chunk jeder Seite: ihre Blöcke und ihr Inhaltsverzeichnis | höchstens 17,3 kB (`zh-CN/development/kpp/anthropic-messages`) | 24 kB pro Seite |

Der Rahmen ändert sich mit dem Code und wächst mit jeder Seite ein wenig, weil das Website-Modul die Seitenleiste und Titel und Beschreibung jeder Seite enthält, sodass die Seitenleiste mit dem Rahmen rendert und nie springt. Deutsch und Chinesisch laden außerdem die Texte der Bibliothek für ihre Sprache, etwa 5 kB. Die Obergrenze ist die gemessene Größe des größten Rahmens mit etwas Luft. Der Inhalt einer Seite hat ein eigenes Budget, sodass eine zu lang gewordene Seite mit ihrer Adresse fehlschlägt, wer auch immer sie bearbeitet; die Lösung sind zwei Seiten.

Die Suche mit ihrem Index, der Toaster und die Grammatiken der Syntaxhervorhebung laden nach dem ersten Rendern und werden nicht mitgezählt. Der Build gibt die Größe der Suche aus, 3,5 kB ohne den Index.

| Teil des Rahmens | Warum er da ist |
| --- | --- |
| React und TanStack Router (`framework`) | Jede Seite |
| `AppFrame`, `Sidebar`, das Navigations-Sheet | Der Rahmen; `Sidebar` bringt das Menü von Base UI mit |
| `Select` | Die Sprach- und Theme-Auswahl am Fuß der Seitenleiste |
| Tooltips, `IconButton` | Die Bedienelemente für Suche und Einklappen |
| Die Seite und ihr Renderer | `Text`, `Link`, `Code`, `CodeBlock` (seine Syntaxhervorhebung lädt nur mit einem Codeblock), `Banner`, `Table`, `ScrollArea`, `Checkbox`, `Separator`, `TableOfContents`, dazu `Badge` und die Datumsformatierung für das Changelog |
| Die Seitenleiste und Titel und Beschreibung jeder Seite | Das Modul `site` der Sprache, sodass die Seitenleiste mit dem Rahmen rendert und nie springt |

- Importe aus `@logicplanes/kaeso-ui` werden zur Build-Zeit auf die Einstiege der einzelnen Komponenten der Bibliothek umgeschrieben (`uiEntryImports` in `apps/docs/vite.config.ts`), sodass der Rahmen nur die Komponenten enthält, die die Website nutzt.
- Seiten werden zur Build-Zeit kompiliert: marked und der Content-Build erreichen nie den Browser.
- Das HTML jeder Seite lädt die Chunks vorab, die ihre Sprache und die Seite selbst brauchen, sodass das erste Rendern nicht auf einen zweiten Round Trip wartet.
- Die Oberflächenschrift wird vorab geladen (`fontPreload`), sodass das erste Rendern in Inter erscheint und nichts neu umbricht, wenn sie eintrifft.
- Bilder tragen Breite und Höhe aus der Datei, sodass die Seite beim Laden nicht springt.

## Die UI-Bibliothek

`pnpm build:packages` baut `@logicplanes/kaeso-ui` und endet mit `packages/ui/scripts/budgets.mjs`, das das gebaute Paket in `packages/ui/dist` gzip-komprimiert misst und den Build fehlschlagen lässt, wenn ein Budget überschritten ist. Eine App bezahlt nur für die Komponenten, die sie importiert, deshalb erfassen die Budgets, was eine Komponente für sich kostet.

| Budget | Was gezählt wird | Gemessen | Budget |
| --- | --- | --: | --: |
| Das Fundament | Die gemeinsamen Chunks, die mindestens 40 % der Komponenten-Einstiegspunkte importieren | 5,3 kB | 8 kB |
| `Button` | `components/button.js` und was es importiert, ohne das Fundament und die Einstiegspunkte anderer Komponenten | 3,0 kB | 3 kB |
| `Menu` | `components/menu.js`, ebenso gezählt | 4,8 kB | 9 kB |
| `DataTable` | `components/data-table.js`, ebenso gezählt | 10,3 kB | 22 kB |
| `styles.css` | Das ganze Stylesheet | 18,0 kB | 28 kB |

Dasselbe Skript lässt den Build außerdem fehlschlagen, wenn:

- ein Modul in `dist` eine Anweisung auf Modulebene hat, die ein Bundler behalten muss, sodass ungenutzte Komponenten nicht wegfallen könnten;
- eine Datei `process.env.NODE_ENV` mit `typeof process` absichert, wodurch Entwicklungsprüfungen in Browser-Bundles bleiben;
- `formatMessage`, `useUiText` oder `useLocale` die Formatierer (`format.js`) mitbringen würden. `formatMessage` allein ist 0,7 kB groß.

## KPP

```bash
cargo bench -p kaeso-codecs --bench codecs
```

Jeder Benchmark durchläuft einen vollständigen Zyklus im selben Dialekt: die Wire-Bytes nach KPP dekodieren, dann KPP wieder in Wire-Bytes kodieren. Die Eingaben werden für jeden Dialekt aus demselben KPP-Inhalt gebaut, sodass die Zahlen vergleichbar sind. Gemessen mit criterion (`--warm-up-time 2 --measurement-time 5`) auf einem Apple M5 Pro, Rust 1.99.0, Release-Profil:

| Benchmark | Anthropic Messages | OpenAI Chat | OpenAI Responses |
| --- | --: | --: | --: |
| Konversation mit 100 Nachrichten (ein System-Prompt, ein Tool, 25 Runden aus Benutzertext, Assistententext mit Tool-Aufruf, Tool-Ergebnis und Assistententext), dekodieren und kodieren | 352 µs (78 MiB/s) | 242 µs (105 MiB/s) | 279 µs (90 MiB/s) |
| Text-Stream mit 2.000 Events, in Chunks zu 512 Bytes eingespeist, dekodieren und kodieren | 3,9 ms (etwa 2,0 µs pro Event, 61 MiB/s) | 10,6 ms (etwa 5,3 µs pro Event, 36 MiB/s) | 8,6 ms (etwa 4,3 µs pro Event, 52 MiB/s) |

Für das Gateway kostet eine lange Konversation in jeder Richtung deutlich weniger als eine Millisekunde und ein gestreamtes Token wenige Mikrosekunden, weit unter den zig Millisekunden zwischen den Tokens jedes Modells. Die Stream-Codecs für Chat und Responses leisten pro Event mehr als der von Anthropic, da jeder Chat-Chunk den Antwort-Envelope wiederholt und jedes Responses-Event eine Sequenznummer trägt und manche das bisher aufgebaute Item wiederholen; dort würde eine Optimierung ansetzen.

Jede Änderung prüft, dass die Benchmarks bauen (`cargo bench -p kaeso-codecs --bench codecs --no-run`); eine Änderung an performancekritischem Codec-Code vergleicht einen vollständigen Lauf mit der Tabelle oben.

## Fuzzing

Die [Fuzz-Targets](testing.md#fuzzing) liefen zuletzt je 10 Minuten, alle fünf parallel auf der Maschine oben, mit `-rss_limit_mb=2048 -max_len=65536`:

| Target | Ausführungen | Abdeckung (Kanten) | Spitzenspeicher | Ergebnis |
| --- | --: | --: | --: | --- |
| `sse` | 7.628.740 | 372 | 707 MB | Kein Absturz |
| `decode_anthropic_messages` | 3.599.755 | 11.047 | 588 MB | Kein Absturz |
| `decode_openai_chat` | 2.044.761 | 11.347 | 570 MB | Kein Absturz |
| `decode_openai_responses` | 1.166.478 | 10.263 | 571 MB | Kein Absturz |
| `decode_openai_embeddings` | 11.245.268 | 4.359 | 668 MB | Kein Absturz |

Der Spitzenspeicher ist der des libFuzzer-Prozesses einschließlich seines Korpus im Speicher; er blieb nach den ersten Minuten konstant, und keine Eingabe erreichte die Grenze von 2 GB. Der Speicher der Bibliothek pro Eingabe ist durch die Größe der Eingabe und die Grenze für SSE-Frames begrenzt (`sse::DEFAULT_MAX_FRAME_BYTES`, 16 MiB).
