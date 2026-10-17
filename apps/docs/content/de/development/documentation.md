---
title: Dokumentation schreiben
description: Wie die Seiten dieser Website geschrieben, abgelegt, verlinkt, geprüft und übersetzt werden.
order: 6
---

Jede Seite dieser Website ist eine Markdown-Datei in `apps/docs/content`, mit einem Ordner pro Sprache (`en`, `de`, `zh-CN`) und denselben Dateinamen in jedem. Nur hier wird Dokumentation geführt; READMEs im Code bleiben kurz und verlinken hierher. Bemerken Personen, die Kaeso nutzen, betreiben oder integrieren, eine Änderung, aktualisiert ihr Pull Request diese Seiten.

## Eine Seite bearbeiten

**Auf GitHub:** am Fuß jeder Seite **Auf GitHub bearbeiten** wählen oder die Datei in `apps/docs/content` öffnen und die Änderung gegen `dev` vorschlagen. GitHub stellt die Seite weitgehend so dar wie die Website, und Links zwischen Seiten funktionieren auch dort.

**Auf dem eigenen Rechner:**

```bash
pnpm build:packages
pnpm --filter kaeso-docs dev
```

Die Website läuft unter http://localhost:4480 und lädt beim Speichern neu; das Terminal nennt alles, was zu beheben ist. `KAESO_DOCS_PORT` wählt einen anderen Port.

| Befehl | Wirkung |
| --- | --- |
| `pnpm --filter kaeso-docs check:content` | Prüft Frontmatter, Links, Anker, Bilder und Adressen jeder Seite und dass jede Sprache dieselben Seiten mit derselben `order` hat, ohne zu bauen |
| `pnpm --filter kaeso-docs build` | Baut die Website nach `apps/docs/dist` und prüft danach die [Bundle-Budgets](performance.md#die-dokumentations-website) |
| `pnpm --filter kaeso-docs preview` | Liefert den Build so aus, wie es ein statischer Host täte |

Der eigene Code der Website wird wie jedes andere Paket getestet ([Tests](testing.md#die-dokumentations-website)).

## Eine Seite

```markdown
---
title: API keys
description: Create keys for apps and scripts, and keep them under control.
order: 5
---

Every request to the model API carries a **key**.

## Create a key
```

| Feld | Pflicht | Bedeutung |
| --- | --- | --- |
| `title` | ja | Die Überschrift der Seite, ihr Name in der Seitenleiste und in der Suche und ihr Browser-Tab |
| `description` | ja | Ein Satz unter dem Titel, höchstens 140 Zeichen; auch die Beschreibung, die Suchmaschinen und Linkvorschauen zeigen |
| `order` | nein | Eine ganze Zahl; kleinere stehen im Ordner weiter vorn. Seiten ohne oder mit gleicher Zahl folgen nach Dateiname |
| `nav` | nein | Eine kürzere Beschriftung für die Seitenleiste |

Jedes andere Feld ist ein Fehler. Der Titel ist die einzige `#`-Überschrift der Seite, deshalb beginnen Überschriften auf der Seite bei `##`. Damit sie sich auf GitHub gut liest, darf eine Seite trotzdem mit `# ` und genau ihrem Titel beginnen; die Website zeigt ihn nur einmal. **Auf dieser Seite** listet die Überschriften `##` und `###` auf.

## Wo eine Seite liegt

| Datei | Adresse | In der Seitenleiste |
| --- | --- | --- |
| `content/en/index.md` | `/` | „Introduction“, an erster Stelle |
| `content/en/console/index.md` | `/console` | Benennt die Gruppe mit ihrem `title`, ordnet sie mit ihrer `order` ein und steht dort als „Overview“ |
| `content/en/console/api-keys.md` | `/console/api-keys` | In dieser Gruppe |
| `content/de/console/api-keys.md` | `/de/console/api-keys` | Dieselbe Seite auf Deutsch |
| `content/zh-CN/console/api-keys.md` | `/zh-CN/console/api-keys` | Dieselbe Seite auf vereinfachtem Chinesisch |

Der Name einer Datei ist ihre Adresse: Kleinbuchstaben, Wörter mit Bindestrichen verbunden (`provider-keys.md`). Wer eine Datei umbenennt oder verschiebt, ändert ihre Adresse in jeder Sprache, und jeder Link darauf muss folgen; der Build nennt die, die es nicht tun. Ein Ordner innerhalb einer Gruppe braucht eine `index.md`, und Ordner sind höchstens drei Ebenen tief verschachtelt. Dateien und Ordner, deren Name mit `_` beginnt, werden nicht veröffentlicht.

## Links

Auf eine andere Seite über ihre Datei verlinken, relativ zur aktuellen Seite und genau so, wie GitHub dem Link folgt:

```markdown
See [rate limits](limits-and-budgets.md#rates) and [the API](../api/index.md).
```

- Ein Anker ist die Überschrift so, wie GitHub sie umformt: kleingeschrieben, ohne Satzzeichen, Leerzeichen als Bindestriche. Eine Überschrift auf derselben Seite ist `[budgets](#budgets)`.
- Ein Link auf eine Seite oder Überschrift, die es nicht gibt, lässt den Build mit Datei und Link fehlschlagen.
- Referenzlinks (`[text][ref]`) werden nicht unterstützt und erscheinen als Text; Links inline schreiben.
- Andere Websites nur mit `http`-, `https`- und `mailto`-Adressen. Repository-Dateien, die keine Seiten sind, auf GitHub verlinken (`https://github.com/logicplanes/kaeso/blob/dev/<path>`) oder als Code nennen: `crates/kaeso-mail`.

## Bilder

Das Bild neben die Seite oder in einen Ordner `images/` daneben legen und mit einem Alternativtext versehen, der sagt, was es zeigt:

```markdown
![The request flow: app, key, model, limits, provider](images/request-flow.svg)
```

PNG, JPEG, GIF, WebP, AVIF und SVG funktionieren. Bilder von anderen Websites werden nicht angezeigt, und ein Bild ohne Alternativtext lässt den Build fehlschlagen. Bilder wählen, die auf hellem und dunklem Hintergrund lesbar sind; ein SVG kann seine Farben selbst mit `@media (prefers-color-scheme: dark)` umschalten. Ein Link auf jede andere Datei in `content/` veröffentlicht diese Datei.

## Hinweisboxen

Die Alerts von GitHub, mit der Art allein in der ersten Zeile:

```markdown
> [!NOTE]
> A key's secret is shown once.
```

`NOTE`, `TIP`, `IMPORTANT`, `WARNING` und `CAUTION` haben jeweils ein eigenes Aussehen und einen eigenen Titel. Sparsam einsetzen.

## Alles andere

Seiten sind GitHub-Flavoured Markdown: **fett**, _kursiv_, ~~durchgestrichen~~, `Code`, Listen, Aufgabenlisten, Tabellen (eine Spalte mit `--:` ist rechtsbündig, für Zahlen), Zitate, horizontale Linien und abgegrenzter Code. Ein Codeblock nennt seine Sprache und kann seine Datei nennen:

````markdown
```ts title="support-bot/src/client.ts"
export const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1" });
```
````

TypeScript, TSX, JavaScript, JSON, Bash (`sh`, `shell`, `console`), Python, YAML und Diff werden hervorgehoben; andere Sprachen erscheinen als einfacher Text. Rohes HTML wird nicht gerendert und erscheint als Text. Codeblöcke enthalten nie Kommentare: Einen Befehl im Satz davor erklären.

Die aktuelle Version als `\{{version}}` schreiben. Der Build ersetzt sie in Text, Code und Frontmatter durch die Version des Releases, sodass keine Seite hinter dem nächsten Release zurückbleibt. Ein Backslash davor lässt die Zeichen stehen, wie auf dieser Seite.

## Schreiben

- **Kaeso so beschreiben, wie der Code es tut.** Einstellungen an der Konfiguration des Gateways prüfen, Beschriftungen am englischen Katalog der Konsole und Verhalten an Gateway, KPP und Konsole. Nie ein Feature dokumentieren, das es nicht gibt.
- **Den aktuellen Stand beschreiben.** Pläne, offene Fragen und To-do-Listen gehören in Issues.
- **Mit dem beginnen, was Lesende brauchen**, ein Gedanke pro Absatz. Tabellen für Referenzdaten, nummerierte Listen für Schritte, Fließtext, wo es um Begründungen geht. Auf die Seite verlinken, die ein Thema behandelt, statt es zu wiederholen.
- **Titel und Überschriften** in normaler Satzschreibung, schlichte Wörter, kurze Sätze, keine Ausrufezeichen.
- **Beschriftungen der Konsole** fett und genau so, wie die Konsole sie zeigt: **API-Schlüssel**, **Schlüssel erstellen**, **Einstellungen** › **Instanz**.
- **Beispiele** nutzen Kaesos eigene Platzhalter (`kaeso.example.com`, `sk-kso-…`, `owner@example.com`), nie echte Schlüssel oder personenbezogene Daten.

## Übersetzen

Eine Änderung an einer englischen Seite wird im selben Pull Request auch in `content/de` und `content/zh-CN` vorgenommen.

- **Gleiche Struktur:** derselbe Dateiname, dieselbe `order`, dieselben Überschriften in derselben Reihenfolge und dieselben Codeblöcke, Byte für Byte. Titel, Beschreibung, Text, Tabellen und Hinweisboxen übersetzen.
- **Links** behalten ihre Datei; ein Anker wird zum Anker der übersetzten Überschrift.
- **Unverändert bleiben** Code, Befehle, Einstellungen (`KAESO_*`), Pfade, Feldnamen, Rechte (`keys.all.view`), Produktnamen und Modell-IDs.
- **Beschriftungen der Konsole** in deren eigener Übersetzung, aus `apps/console/src/i18n/locales/de` und `apps/console/src/i18n/locales/zh-CN`.
- **Natürlich schreiben**, nicht Wort für Wort, und jede Tatsache und jede Zahl behalten.

Hausstil und Glossar stehen unter [Übersetzungen](translations.md#hausstil). Zwei Regeln zählen auf Seiten besonders:

| | Deutsch | Vereinfachtes Chinesisch |
| --- | --- | --- |
| Schritte | Infinitiv, Verb am Ende: „Schlüssel erstellen“ | Zuerst ein Verb: “创建密钥” |
| Beschreibungen | Ein Satz | Ein Satz, endet auf “。” |

Die eigenen Texte der Website, etwa **Auf GitHub bearbeiten** oder die Suche, stehen in `apps/docs/src/i18n/messages/en.ts`, `de.ts` und `zh-CN.ts`, so typisiert, dass eine fehlende Übersetzung den Typecheck scheitern lässt.
