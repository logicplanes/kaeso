---
title: Dokumentations-Website
description: Wie aus den Markdown-Seiten in apps/docs/content die statische Website unter docs.kaeso.app wird.
order: 3
---

Die Dokumentation ist das Paket `kaeso-docs` in `apps/docs`: Markdown-Seiten in drei Sprachen, gebaut zu einer statischen Website, die aussieht und funktioniert wie die Konsole und nur mit `@logicplanes/kaeso-ui` gezeichnet wird. Wie man eine Seite schreibt, steht unter [Dokumentation schreiben](../documentation.md); diese Seite beschreibt, wie die Website funktioniert.

```text
content/<language>/**/*.md ─▶ scripts/content (at build time, in Vite) ─▶ virtual modules ─▶ src/ (React, @logicplanes/kaeso-ui) ─▶ dist/
                              frontmatter · marked's lexer · links,          site/<language>: sidebar, titles, previous/next, a loader per page
                              anchors and images checked · search words      page/<language>/<path>: one page's blocks, its own chunk
                              the same pages in every language               search/<language>: the index, loaded with the search
```

## Inhaltsmodell

| Baustein | Ort | Regel |
| --- | --- | --- |
| Eine Sprache | `content/en`, `content/de`, `content/zh-CN` | Englisch unter `/`, die anderen unter `/de` und `/zh-CN`; jede Sprache hat dieselben Dateien mit derselben `order` |
| Eine Seite | `content/<language>/<folders>/<name>.md` | Ihre Adresse ist ihr Pfad: `content/en/console/api-keys.md` ist `/console/api-keys`; `index.md` ist die Seite ihres Ordners |
| Die Startseite | `content/<language>/index.md` | Pflicht |
| Eine Gruppe in der Seitenleiste | Ein Ordner in einer Sprache | Benannt nach dem `title` seiner `index.md` oder nach dem Ordnernamen; platziert nach der `order` dieser `index.md` |
| Eine verschachtelte Seite | Ein Ordner innerhalb einer Gruppe, mit `index.md` | Ihre Seiten werden darunter eingerückt; höchstens drei Ordnerebenen |
| Reihenfolge innerhalb eines Ordners | Frontmatter `order` | Zuerst die `index.md` des Ordners, dann nach `order`, dann nach Beschriftung |
| Die Beschriftung einer Seite in der Seitenleiste | Frontmatter `nav` | Standard ist der Titel; für `index.md` das „Übersicht“ der Sprache (auf der Startseite „Einführung“) |
| Bilder und Dateien | Überall in `content/` | Nur veröffentlicht, wenn eine Seite sie zeigt oder verlinkt, mit ihrer zur Build-Zeit gelesenen Größe |
| Entwürfe und private Notizen | Namen, die mit `_` oder `.` beginnen | Nicht Teil der Website |

Das Frontmatter hat genau vier Felder: `title` und `description` (ein kurzer Satz, höchstens 140 Zeichen) sind Pflicht, `order` und `nav` optional. Jedes andere Feld ist ein Fehler, sodass ein Tippfehler nie stillschweigend wirkungslos bleibt.

## Der Build

`apps/docs/scripts/content` macht aus den Seiten die Module der Website:

| Datei | Aufgabe |
| --- | --- |
| `frontmatter.ts` | Die vier Felder, mit einer Meldung für jeden Fehler; ein Satz pro Beschreibung, mit lateinischer und chinesischer Interpunktion |
| `slug.ts` | GitHubs Überschriften-Anker (`## Rate limits` ist `#rate-limits`, Wiederholungen bekommen `-1`, `-2`) und Adressen aus Dateinamen |
| `compile.ts` | Eine Seite: der Lexer von marked mit abgeschaltetem Roh-HTML, dann das Seitenformat aus `src/markdown/ast.ts`; geprüfte Links; Callouts; Suchwörter |
| `site.ts` | Eine Sprache: Adressen, die Seitenleiste, vorherige und nächste Seite, relative Links so aufgelöst, wie GitHub sie auflöst, Bilder und ihre Größen, Suche |
| `docs.ts` | Alle Sprachen: je eine Website mit ihrem Präfix und ihren Beschriftungen, und die Prüfungen, dass sie dieselben Seiten in derselben Reihenfolge enthalten |
| `plugin.ts` | Das Vite-Plugin: virtuelle Module, Neuladen bei einer Änderung in der Entwicklung, eine HTML-Datei pro Seite und Sprache, `404.html`-Dateien und die Sitemap |
| `check.ts` | `pnpm --filter kaeso-docs check:content`: dieselben Prüfungen, ohne die App zu bauen |

Jedes Problem nennt seine Datei: ein Link auf eine Seite oder Überschrift, die es nicht gibt, ein Bild ohne Alternativtext oder von außerhalb von `content/`, ein Link mit einem anderen Schema als http, https oder mailto, eine zweite `#`-Überschrift, zwei Dateien mit derselben Adresse, eine Seite, die in einer Sprache fehlt oder dort eine andere `order` hat. In der Entwicklung werden sie ausgegeben und die Seite rendert trotzdem; der Build und `check:content` schlagen damit fehl.

### Sicherheit

Seiten werden im Browser auf GitHub bearbeitet, daher behandelt der Build sie als nicht vertrauenswürdigen Text. marked tokenisiert nur, mit abgeschalteten HTML-Tokenizern: `<script>`, `<img onerror>` und jedes andere Tag bleiben wörtlicher Text. Link-Referenzen sind abgeschaltet, und Links behalten nur http, https und mailto, nachdem Leerraum und Steuerzeichen entfernt wurden, sodass `java\nscript:` erkannt wird. Bilder kommen nur aus `content/`. Der Browser erhält einfache Objekte, nie HTML, und nichts setzt `innerHTML`. `scripts/content/compile.test.ts` enthält die Fälle.

### Links, die auf GitHub und auf der Website funktionieren

Ein Link wird zwischen Dateien geschrieben, so wie GitHub ihm folgt: `[keys](../console/api-keys.md#create-a-key)`. Der Build löst ihn relativ zum Ordner der Datei auf, prüft, dass Seite und Überschrift existieren, und schreibt die Adresse der Website in der Sprache der Seite (`/de/console/api-keys#schlüssel-erstellen`). Überschriften verwenden GitHubs Anker, sodass derselbe Anker an beiden Orten funktioniert; eine übersetzte Seite verlinkt auf den Anker der übersetzten Überschrift.

## Die App

| Ordner | Aufgabe |
| --- | --- |
| `src/router.tsx` | Ein Routenbaum pro Sprache: ein Layout, das das Website-Modul der Sprache und die Texte der Bibliothek lädt, und eine Route pro Adresse |
| `src/site/` | Lädt das Site-Modul einer Sprache, ihren Suchindex und die Texte der Bibliothek, jeweils einmal |
| `src/page/` | Die Seite (Titel, Beschreibung, Inhalt, vorherige und nächste Seite, „Auf GitHub bearbeiten“, „Auf dieser Seite“), „Nicht gefunden“ und eine Seite, die nicht laden kann |
| `src/markdown/` | Das Seitenformat (`ast.ts`) und seine Darstellung mit Komponenten der Bibliothek (`render.tsx`) |
| `src/shell/` | `AppFrame` mit der Seitenleiste, der Suche, dem Sprachmenü, dem Theme, Links über den Router, Toasts |
| `src/i18n/` | Die Sprachen und ihre Adressen, die gespeicherte Präferenz und die eigenen Texte der Website in `messages/` |

Der Rahmen ist der der Konsole: `AppFrame` mit `Sidebar`, die Wortmarke allein oben links, 28px nach innen und 28px nach unten, das Panel eingerückt im Chrome, und unter 1024px die Leiste für Telefone und das Navigations-Sheet. Eine Seite ist aufgebaut wie die Einstellungsseiten der Konsole: ein Titel in `title-lg` und eine Beschreibung in `body-lg` in einer Spalte von 640px; Text ist `body-lg`, Überschriften `title` (`##`) und `title-sm` (`###`), Code in `CodeBlock`, Callouts in `Banner`, Tabellen in `Table` innerhalb einer horizontalen `ScrollArea`. Ab 1280px steht „Auf dieser Seite“ (`TableOfContents`) rechts; seine Spalte bleibt auch auf Seiten ohne es erhalten, sodass sich der Text zwischen Seiten nie verschiebt.

Wenn ein Link mit der Maus berührt oder fokussiert wird, lädt der Chunk der Seite (`defaultPreload: "intent"`), sodass Seiten sofort öffnen. Die Suche (`CommandPalette`) und ihr Index laden einen Moment nach dem ersten Rendern; ⌘K oder Strg+K öffnet sie, und was vor dem Laden getippt wurde, wird zu ihrer Suchanfrage. Sie findet Seiten nach Titel, Beschreibung und Wörtern, Überschriften nach ihren eigenen Wörtern, und bietet das Theme und die anderen Sprachen an.

Der erste Besuch einer englischen Adresse folgt den Sprachen des Browsers und wechselt ohne Navigation nach `/de` oder `/zh-CN`; die Wahl einer Sprache im Menü oder in der Suche öffnet dieselbe Seite in dieser Sprache und wird als `kaeso-docs:locale` gespeichert. Das Theme ist das der Bibliothek: System, hell oder dunkel, vom Theme-Skript vor dem ersten Rendern angewendet.

Jeder Fehler ist ein Toast: Eine Seite, deren Chunk nicht laden kann, behält den Rahmen, zeigt einen leeren Zustand mit „Erneut versuchen“ und sagt in einem Toast, was passiert ist; eine Suche, die nicht laden kann, schließt sich mit einem Toast.

## Designentscheidungen

- **Frontmatter statt einer Datei für die Seitenleiste.** Der Platz einer Seite ergibt sich aus ihrem Ordner und ihrer `order`; Name und Platz eines Abschnitts aus seiner `index.md`. Eine Seite auf GitHub zu verschieben oder hinzuzufügen betrifft eine Datei pro Sprache, nie eine zweite Datei, die dazu passen muss.
- **Ein führendes `# Title`.** GitHub zeigt das Frontmatter als kleine Tabelle, daher darf eine Seite mit `# ` und ihrem Titel beginnen, damit sie sich dort gut liest; die Website lässt diese Überschrift weg. Jede andere `#`-Überschrift ist ein Fehler.
- **Callouts.** GitHubs fünf Alerts werden zu `Banner`s: Note neutral, Tip success, Important info, Warning und Caution warning. Caution bekommt nicht den Ton danger, weil das Danger-Banner `role="alert"` hat, das ein Screenreader beim Öffnen der Seite ansagt.
- **Tabellen** sind mindestens 640px breit innerhalb einer horizontalen `ScrollArea`: Auf einem Telefon scrollen sie seitwärts, statt jede Spalte auf ein Wort pro Zeile zusammenzudrücken.
- **Kein TanStack Query.** Die Website lädt nichts außer ihren eigenen Chunks, die die Loader des Routers laden und behalten (`defaultStaleTime: Infinity`).
- **Absichtlich nicht unterstützt:** Roh-HTML, MDX oder Komponenten in Seiten, Fußnoten, Bilder von anderen Websites, Definitionen von Link-Referenzen.
- **Code** wird für die Sprachen der Bibliothek hervorgehoben (TypeScript, TSX, JavaScript, JSON, Bash, Python, YAML, diff und ihre Aliasse); alles andere ist einfacher Text im selben Block.
- **Eine Version.** Die Seiten beschreiben das neueste Release; es gibt keine versionierten Kopien.

## Die Build-Ausgabe

`pnpm --filter kaeso-docs build` schreibt `apps/docs/dist`: die App, eine `<path>.html` für jede Seite in jeder Sprache mit ihrem `lang`, `<title>`, Beschreibung, Open-Graph-Titel und -Beschreibung, kanonischer Adresse und `hreflang`-Alternativen (`/de/console/api-keys` ist `de/console/api-keys.html`), eine `404.html` pro Sprache, `sitemap.xml`, `robots.txt` und `CNAME`. Jeder statische Host kann sie ausliefern: Eine Anfrage nach `/console/api-keys` bekommt `console/api-keys.html`, und eine unbekannte Adresse bekommt `404.html`, die „Nicht gefunden“ im Rahmen zeigt. Der Maintainer veröffentlicht sie unter docs.kaeso.app.

`vite.config.ts` macht, was die der Konsole macht: Es schreibt Importe aus `@logicplanes/kaeso-ui` auf die Einstiegspunkte der einzelnen Komponenten der Bibliothek um (`uiEntryImports`) und liefert das Theme-Skript als `/theme-init.js` aus; außerdem fügt es die `theme-color`-Meta-Tags hinzu und lädt die Schrift der Oberfläche vor. Der Build endet mit `scripts/budget.ts`, das den Rahmen und jede Seite gegen ihre [Bundle-Budgets](../performance.md#die-dokumentations-website) prüft, und `pnpm --filter kaeso-docs licenses:check` (`scripts/licenses.ts`) prüft danach die Lizenz jedes Pakets darin.
