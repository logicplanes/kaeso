---
title: Dokumentations-Website
description: Wie aus den Markdown-Seiten in apps/docs/content die statische Website unter docs.kaeso.app wird.
order: 3
---

Die Dokumentation ist das Paket `kaeso-docs` in `apps/docs`: Markdown-Seiten in drei Sprachen, gebaut zu einer statischen Website, die aussieht und funktioniert wie die Konsole und nur mit `@logicplanes/kaeso-ui` gezeichnet ist. [Dokumentation schreiben](../documentation.md) erklärt, wie eine Seite geschrieben wird; diese Seite erklärt, wie die Website funktioniert.

```text
content/<language>/**/*.md ─▶ scripts/content (at build time, in Vite) ─▶ virtual modules ─▶ src/ (React, @logicplanes/kaeso-ui) ─▶ dist/
                              frontmatter · marked's lexer · links,          site/<language>: sidebar, titles, previous/next, a loader per page
                              anchors and images checked · search words      page/<language>/<path>: one page's blocks, its own chunk
                              the same pages in every language               search/<language>: the index, loaded with the search
```

## Inhaltsmodell

| Element | Ort | Regel |
| --- | --- | --- |
| Eine Sprache | `content/en`, `content/de`, `content/zh-CN` | Englisch unter `/`, die anderen unter `/de` und `/zh-CN`; jede Sprache hat dieselben Dateien mit derselben `order` |
| Eine Seite | `content/<language>/<folders>/<name>.md` | Ihre Adresse ist ihr Pfad: `content/en/console/api-keys.md` ist `/console/api-keys`; `index.md` ist die Seite ihres Ordners |
| Die Startseite | `content/<language>/index.md` | Pflicht |
| Eine Gruppe in der Seitenleiste | Ein Ordner in einer Sprache | Benannt nach dem `title` ihrer `index.md` oder nach dem Ordnernamen; platziert nach der `order` dieser `index.md` |
| Eine verschachtelte Seite | Ein Ordner in einer Gruppe, mit `index.md` | Ihre Seiten erscheinen eingerückt darunter; höchstens drei Ordnerebenen |
| Reihenfolge in einem Ordner | Frontmatter `order` | Zuerst die `index.md` des Ordners, dann nach `order`, dann nach Beschriftung |
| Beschriftung einer Seite in der Seitenleiste | Frontmatter `nav` | Standard ist der Titel; für `index.md` das „Übersicht“ der Sprache (für die Startseite „Einführung“) |
| Bilder und Dateien | Überall in `content/` | Nur veröffentlicht, wenn eine Seite sie zeigt oder verlinkt; ihre Größe wird beim Build gelesen |
| Entwürfe und private Notizen | Namen, die mit `_` oder `.` beginnen | Nicht Teil der Website |

Das Frontmatter hat genau vier Felder: `title` und `description` (ein kurzer Satz, höchstens 140 Zeichen) sind Pflicht, `order` und `nav` optional. Jedes andere Feld ist ein Fehler, damit ein Tippfehler nie stillschweigend wirkungslos bleibt.

## Der Build

`apps/docs/scripts/content` macht aus den Seiten die Module der Website:

| Datei | Aufgabe |
| --- | --- |
| `frontmatter.ts` | Die vier Felder, mit einer Meldung für jeden Fehler; ein Satz pro Beschreibung, mit lateinischer und chinesischer Zeichensetzung |
| `slug.ts` | Die Überschriften-Anker von GitHub (`## Rate limits` ist `#rate-limits`, Wiederholungen bekommen `-1`, `-2`) und Adressen aus Dateinamen |
| `compile.ts` | Eine Seite: der Lexer von marked mit abgeschaltetem Roh-HTML, dann das Seitenformat aus `src/markdown/ast.ts`; geprüfte Links; Callouts; Suchwörter |
| `site.ts` | Eine Sprache: Adressen, die Seitenleiste, vorherige und nächste Seite, relative Links aufgelöst wie auf GitHub, Bilder und ihre Größen, die Suche |
| `docs.ts` | Alle Sprachen: je eine Website mit Präfix und Beschriftungen, und die Prüfung, dass sie dieselben Seiten in derselben Reihenfolge enthalten |
| `changelog.ts` | Die Changelog-Seite, in jeder Sprache aus `CHANGELOG.md` erzeugt ([das Changelog](#das-changelog)) |
| `plugin.ts` | Das Vite-Plugin: virtuelle Module, Neuladen bei Änderungen in der Entwicklung, eine HTML-Datei pro Seite und Sprache mit ihrem Head für Suchmaschinen und Linkvorschauen, `404.html`-Dateien und die Sitemap |
| `check.ts` | `pnpm --filter kaeso-docs check:content`: dieselben Prüfungen, ohne die App zu bauen |

Jedes Problem nennt seine Datei:

- ein Link auf eine Seite oder Überschrift, die es nicht gibt;
- ein Bild ohne Alternativtext oder von außerhalb von `content/`;
- ein Link mit einem anderen Schema als http, https oder mailto;
- eine zweite `#`-Überschrift oder zwei Dateien mit derselben Adresse;
- eine Seite, die in einer Sprache fehlt oder dort eine andere `order` hat.

In der Entwicklung werden sie ausgegeben, und die Seite wird trotzdem gerendert; der Build und `check:content` schlagen bei ihnen fehl.

### Sicherheit

Seiten werden im Browser auf GitHub bearbeitet, daher behandelt der Build sie als nicht vertrauenswürdigen Text:

- marked zerlegt nur in Tokens, mit abgeschalteten HTML-Tokenizern: `<script>`, `<img onerror>` und jedes andere Tag bleiben wörtlicher Text;
- Link-Referenzen sind abgeschaltet, und Links behalten nur http, https und mailto, geprüft nach dem Entfernen von Leer- und Steuerzeichen, sodass `java\nscript:` auffällt;
- Bilder kommen nur aus `content/`;
- der Browser erhält einfache Objekte, nie HTML, und nichts setzt `innerHTML`.

`scripts/content/compile.test.ts` enthält die Fälle.

### Links, die auf GitHub und auf der Website funktionieren

Ein Link wird zwischen Dateien geschrieben, so wie GitHub ihm folgt: `[keys](../console/api-keys.md#create-a-key)`. Der Build löst ihn relativ zum Ordner der Datei auf, prüft, dass Seite und Überschrift existieren, und schreibt die Adresse der Website in der Sprache der Seite (`/de/console/api-keys#schlüssel-erstellen`). Überschriften verwenden die Anker von GitHub, daher funktioniert derselbe Anker an beiden Orten; eine übersetzte Seite verlinkt auf den Anker der übersetzten Überschrift.

### Das Changelog

`/changelog` ist in jeder Sprache die einzige Seite, die keine Datei in `content/` ist. Der Build erzeugt sie aus `CHANGELOG.md` im Wurzelverzeichnis des Repositorys, daher hat das Changelog eine einzige Quelle und wird nie von Hand übersetzt.

- **Zerlegen.** `history` in `scripts/changelog.mjs`, das auch bei einem Release das Changelog fortschreibt, teilt die Datei in Releases, Abschnitte wie `### Added` und Themen wie `#### Model API`. `changelog.ts` kompiliert jeden Teil wie eine Seite und folgt dabei den Link-Referenzen der Datei. Ein Link auf etwas anderes als eine vollständige Adresse oder eine Version auf der Seite lässt den Build scheitern.
- **Sprachen.** Die eigenen Wörter der Seite, auch die Abschnittsnamen, gehören zur Website und sind übersetzt. Die Einträge bleiben Englisch, auf den deutschen und chinesischen Seiten mit `lang="en"` markiert, die das unter ihrer Beschreibung sagen.
- **Releases.** Der Anker jeder Version ist `v` und die Version, wie ihr Git-Tag, in jeder Sprache: `/de/changelog#v{{version}}`. Das neueste stabile Release ist wie auf GitHub als Latest markiert, Pre-Releases als solche. Jedes Release zeigt sein Datum in der Sprache der Seite und verlinkt auf sein GitHub-Release und die Änderungen seit dem vorherigen Release.
- **Unreleased.** `[Unreleased]` entfällt, wenn es keine Zeilen hat, wie auf den Release-Tags, aus denen die Websites gebaut werden. Mit Zeilen steht es zuerst und sagt, dass sie noch in keinem Release sind.
- **Platzierung.** Die Seite folgt in der Seitenleiste auf die Einführung, nennt den Release-Feed von GitHub für Feedreader und erscheint in den Suchergebnissen zuletzt.

Die Unit-Tests und die E2E-Suite lesen `e2e/fixtures/CHANGELOG.md`; `KAESO_DOCS_CHANGELOG` wählt eine andere Datei.

## Die App

| Ordner | Aufgabe |
| --- | --- |
| `src/router.tsx` | Ein Routenbaum pro Sprache: ein Layout, das das Website-Modul der Sprache und die Texte der Bibliothek lädt, und eine Route pro Adresse |
| `src/site/` | Lädt das Website-Modul einer Sprache, ihren Suchindex und die Texte der Bibliothek, jeweils einmal |
| `src/page/` | Die Seite (Titel, Beschreibung, Inhalt, vorherige und nächste Seite, Auf GitHub bearbeiten, „Auf dieser Seite“), „nicht gefunden“ und eine Seite, die nicht laden kann |
| `src/markdown/` | Das Seitenformat (`ast.ts`) und seine Darstellung mit Komponenten der Bibliothek (`render.tsx`) |
| `src/shell/` | `AppFrame` mit Seitenleiste, Suche, Sprach- und Theme-Auswahl, Links über den Router, Toasts |
| `src/i18n/` | Die Sprachen und ihre Adressen, die gespeicherte Präferenz und die eigenen Wörter der Website in `messages/` |

**Der Rahmen** ist der der Konsole: `AppFrame` mit dem Link „Zum Inhalt springen“ und `Sidebar`, die Wortmarke allein oben links (28px nach innen und 28px nach unten), das Panel eingerückt im Chrome und unter 1024px die Handyleiste und das Navigations-Sheet; die Handyleiste enthält den Suchbutton neben dem Navigationsschalter.

**Eine Seite** ist aufgebaut wie die Einstellungsseiten der Konsole: ein Titel in `title-lg` und eine Beschreibung in `body-lg` in einer 640px breiten Spalte. Fließtext ist `body-lg`, Überschriften sind `title` (`##`) und `title-sm` (`###`), Code steht in `CodeBlock`, Callouts sind `Banner`, Tabellen sind `Table` in einer horizontalen `ScrollArea`. Ab 1280px steht „Auf dieser Seite“ (`TableOfContents`) rechts daneben; seine Spalte bleibt auch auf Seiten ohne ihn erhalten, sodass sich der Text zwischen Seiten nie verschiebt.

**Laden.** Hovern oder Fokussieren eines Links lädt den Chunk der Seite (`defaultPreload: "intent"`), sodass Seiten sofort öffnen. Die Suche (`CommandPalette`) und ihr Index laden kurz nach dem ersten Zeichnen. Der Suchbutton in der Seitenleiste oder der Handyleiste, ⌘K oder Strg+K öffnet sie, und was vor dem Laden getippt wurde, wird ihre Suchanfrage. Sie findet Seiten über Titel, Beschreibung und Wörter, Überschriften über ihre eigenen Wörter, und bietet das Theme und die anderen Sprachen an.

**Sprache und Theme.** Der erste Besuch einer englischen Adresse folgt den Sprachen des Browsers und wechselt ohne Navigation nach `/de` oder `/zh-CN`. Eine Sprache im Menü oder in der Suche zu wählen, öffnet dieselbe Seite in dieser Sprache und wird als `kaeso-docs:locale` gespeichert. Das Theme ist das der Bibliothek: System, hell oder dunkel, vor dem ersten Zeichnen vom Theme-Skript angewendet.

**Fehler** sind Toasts. Eine Seite, deren Chunk nicht lädt, behält den Rahmen, zeigt einen leeren Zustand mit „Erneut versuchen“ und erklärt in einem Toast, was passiert ist; eine Suche, die nicht lädt, schließt sich mit einem Toast.

## Designentscheidungen

- **Frontmatter statt einer Datei für die Seitenleiste.** Der Platz einer Seite ist ihr Ordner und ihre `order`; Name und Platz eines Abschnitts sind die seiner `index.md`. Eine Seite auf GitHub zu verschieben oder hinzuzufügen, betrifft eine Datei pro Sprache, nie eine zweite Datei, die übereinstimmen muss.
- **Ein führendes `# Titel`.** GitHub zeigt das Frontmatter als kleine Tabelle, daher darf eine Seite mit `# ` und ihrem Titel beginnen, damit sie dort gut lesbar ist; die Website lässt diese Überschrift weg. Jede andere `#`-Überschrift ist ein Fehler.
- **Callouts.** Die fünf Hinweise von GitHub werden zu `Banner`: Note neutral, Tip success, Important info, Warning und Caution warning. Caution nimmt nicht den Ton danger, weil das Danger-Banner `role="alert"` hat, das ein Screenreader beim Öffnen der Seite vorliest.
- **Tabellen** halten Wörter ganz. Eine Spalte, deren Zellen alle kurz sind (höchstens 20 Zeichen), bleibt einzeilig, andere Spalten sind mindestens 140px breit, und eine Tabelle, die breiter als die Seite ist, scrollt seitwärts, statt ihre Spalten zu stauchen.
- **Kein TanStack Query.** Die Website lädt nichts außer ihren eigenen Chunks, die die Loader des Routers laden und behalten (`defaultStaleTime: Infinity`).
- **Bewusst nicht unterstützt:** Roh-HTML, MDX oder Komponenten in Seiten, Fußnoten, Bilder von anderen Websites, Definitionen von Link-Referenzen.
- **Code** wird für die Sprachen der Bibliothek hervorgehoben (TypeScript, TSX, JavaScript, JSON, Bash, Python, YAML, diff und ihre Aliasse); alles andere ist Klartext im selben Block.
- **Eine Version.** Die Seiten beschreiben das neueste Release; es gibt keine versionierten Kopien.

## Die Build-Ausgabe

`pnpm --filter kaeso-docs build` schreibt `apps/docs/dist`:

| Ausgabe | Was sie ist |
| --- | --- |
| Die App | Die JavaScript- und CSS-Chunks |
| `<path>.html` | Eine Datei für jede Seite in jeder Sprache: `/de/console/api-keys` ist `de/console/api-keys.html` |
| `404.html` | Eine pro Sprache, als `noindex` markiert; zeigt „nicht gefunden“ im Rahmen |
| `sitemap.xml` | Jede Seite in jeder Sprache mit ihren `hreflang`-Alternativen, ohne `lastmod`, da der Build nicht weiß, wann sich eine Seite zuletzt geändert hat |
| `robots.txt` | Erlaubt alles und nennt die Sitemap |
| `site.webmanifest`, die Icons, `og.png`, `CNAME` | Statische Dateien aus `public/` |
| `csp.txt`, `_headers` | Die [Content Security Policy](../releasing.md#content-security-policy) der Website, die jede Seite auch in einem `<meta>` trägt |

Der Head einer Seite enthält ihr `lang`, `<title>` und ihre Beschreibung, ihre kanonische Adresse, `hreflang`-Alternativen in den drei Sprachen und `x-default`, Open-Graph- und Twitter-Tags mit `og.png`, ihr `og:locale` mit den anderen beiden als Alternativen sowie strukturierte Daten: `WebSite` auf der Startseite einer Sprache, `TechArticle` auf jeder anderen Seite.

Jeder statische Host kann die Ausgabe ausliefern: Eine Anfrage nach `/console/api-keys` bekommt `console/api-keys.html`, eine unbekannte Adresse `404.html`. Der Maintainer veröffentlicht sie unter docs.kaeso.app.

`vite.config.ts` macht, was die der Konsole macht: Es schreibt Importe aus `@logicplanes/kaeso-ui` auf die Einstiegspunkte der einzelnen Komponenten um (`uiEntryImports`) und liefert das Theme-Skript als `/theme-init.js` aus. Außerdem fügt es die Meta-Tags `theme-color` hinzu und lädt die Schrift der Oberfläche vorab.

Der Build endet mit zwei Prüfungen: `scripts/budget.ts` prüft den Rahmen und jede Seite gegen ihre [Bundle-Budgets](../performance.md#die-dokumentations-website), und `scripts/check-site.mjs` prüft die Ausgabe ([die Websites](../releasing.md#die-websites)). `pnpm --filter kaeso-docs licenses:check` (`scripts/licenses.ts`) prüft danach die Lizenz jedes Pakets im Build.
