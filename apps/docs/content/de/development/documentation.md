---
title: Dokumentation schreiben
description: Wie die Seiten dieser Website geschrieben, abgelegt, verlinkt, geprüft und übersetzt werden.
order: 6
---

Jede Seite dieser Website ist eine Markdown-Datei in `apps/docs/content`, in einem Ordner pro Sprache: `content/en`, `content/de` und `content/zh-CN`, mit denselben Dateinamen in jedem. Nur hier wird Dokumentation gepflegt; der Code hat kurze `README.md`-Dateien, die hierher verlinken. Eine Änderung, die Personen bemerken, die Kaeso nutzen, betreiben oder integrieren, aktualisiert diese Seiten im selben Pull Request.

## Eine Seite bearbeiten

**Auf GitHub:** Am Fuß jeder Seite „Auf GitHub bearbeiten“ wählen oder die Datei in `apps/docs/content` öffnen, bearbeiten und die Änderung gegen `dev` vorschlagen. GitHub zeigt die Seite weitgehend so, wie die Website es tun wird, und auch dort funktioniert jeder Link zwischen Seiten.

**Auf dem eigenen Rechner:**

```bash
pnpm build:packages
pnpm --filter kaeso-docs dev
```

Die Website läuft unter http://localhost:4480; Seiten laden beim Speichern neu, und das Terminal nennt alles, was zu korrigieren ist. `KAESO_DOCS_PORT` wählt einen anderen Port.

| Befehl | Was er tut |
| --- | --- |
| `pnpm --filter kaeso-docs check:content` | Prüft Frontmatter, Links, Anker, Bilder und Adressen jeder Seite und dass jede Sprache dieselben Seiten hat, ohne zu bauen |
| `pnpm --filter kaeso-docs build` | Baut die Website nach `apps/docs/dist` und prüft dann die [Bundle-Budgets](performance.md#die-dokumentations-website) |
| `pnpm --filter kaeso-docs preview` | Stellt den Build bereit, wie es ein statischer Host täte |

Der eigene Code der Website wird getestet wie jedes andere Paket ([Tests](testing.md#die-dokumentations-website)).

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

| Feld | Erforderlich | Was es ist |
| --- | --- | --- |
| `title` | ja | Der Titel der Seite, ihr Name in der Seitenleiste und in der Suche und ihr Browser-Tab |
| `description` | ja | Ein kurzer Satz unter dem Titel, höchstens 140 Zeichen; zugleich die Beschreibung der Seite für Suchmaschinen und Linkvorschauen |
| `order` | nein | Eine ganze Zahl: Niedrigere Werte kommen unter den Seiten ihres Ordners zuerst; Seiten ohne folgen, nach Titel sortiert |
| `nav` | nein | Eine kürzere Beschriftung für die Seitenleiste |

Mehr gehört nicht ins Frontmatter; ein unbekanntes Feld ist ein Fehler. Überschriften auf der Seite beginnen bei `##`: Der Titel ist das einzige `#` der Seite. Damit sie sich auf GitHub gut liest, darf eine Seite mit `# ` und genau ihrem Titel beginnen; die Website zeigt ihn nur einmal.

## Wo eine Seite liegt

| Datei | Adresse | In der Seitenleiste |
| --- | --- | --- |
| `content/en/index.md` | `/` | „Einführung“, ganz oben |
| `content/en/console/index.md` | `/console` | Benennt die Gruppe mit ihrem `title` und ordnet sie mit ihrem `order` ein; als „Übersicht“ aufgeführt |
| `content/en/console/api-keys.md` | `/console/api-keys` | In dieser Gruppe |
| `content/de/console/api-keys.md` | `/de/console/api-keys` | Dieselbe Seite auf Deutsch |
| `content/zh-CN/console/api-keys.md` | `/zh-CN/console/api-keys` | Dieselbe Seite auf vereinfachtem Chinesisch |

Der Name einer Datei ist ihre Adresse: kleingeschrieben, Wörter mit Bindestrichen verbunden (`provider-keys.md`). Eine Datei zu verschieben oder umzubenennen ändert ihre Adresse in jeder Sprache, und jeder Link darauf muss folgen; der Build nennt die, die es nicht tun. Ein Ordner innerhalb einer Gruppe braucht eine `index.md` und gliedert seine Seiten darunter, höchstens drei Ordner tief. Dateien und Ordner, deren Namen mit `_` beginnen, werden nicht veröffentlicht.

## Links

Auf eine andere Seite über ihre Datei verlinken, relativ zur Seite, die gerade geschrieben wird, genau so, wie GitHub dem Link folgt:

```markdown
See [rate limits](limits-and-budgets.md#rates) and [the API](../api/index.md).
```

- `#rates` ist der Anker der Überschrift, wie GitHub ihn bildet: kleingeschrieben, Satzzeichen entfernt, Leerzeichen als Bindestriche. Eine Überschrift auf derselben Seite ist `[budgets](#budgets)`.
- Ein Link auf eine Seite oder Überschrift, die es nicht gibt, lässt den Build fehlschlagen, mit Datei und Link.
- Andere Websites: nur `https://…` und `mailto:`. Dateien des Repositorys, die keine Seiten sind, werden auf GitHub verlinkt, als `https://github.com/logicplanes/kaeso/blob/dev/<path>`, oder als Code genannt: `crates/kaeso-mail`.

## Bilder

Das Bild neben die Seite legen oder in einen Ordner `images/` daneben und es mit einem relativen Pfad und einem Alternativtext zeigen, der sagt, was es zeigt:

```markdown
![The request flow: app, key, model, limits, provider](images/request-flow.svg)
```

PNG, JPEG, GIF, WebP, AVIF und SVG funktionieren. Bilder von anderen Websites werden nicht angezeigt, und ein Bild ohne Alternativtext lässt den Build fehlschlagen. Ein Bild wählen, das auf hellem und dunklem Hintergrund lesbar ist; ein SVG kann seine Farben mit `@media (prefers-color-scheme: dark)` selbst umschalten. Ein Link auf eine beliebige andere Datei in `content/` veröffentlicht diese Datei.

## Hinweisboxen

Die Alerts von GitHub, mit der Art allein in der ersten Zeile:

```markdown
> [!NOTE]
> A key's secret is shown once.
```

`NOTE`, `TIP`, `IMPORTANT`, `WARNING` und `CAUTION` haben jeweils ein eigenes Aussehen und einen eigenen Titel. Sparsam einsetzen.

## Alles andere

GitHub Flavored Markdown: **fett**, _kursiv_, ~~durchgestrichen~~, `code`, Listen und nummerierte Listen, Aufgabenlisten, Tabellen (eine Spalte `--:` ist rechtsbündig, für Zahlen), Blockzitate, horizontale Linien und abgegrenzter Code. Ein Codeblock nennt seine Sprache und darf seine Datei nennen:

````markdown
```ts title="support-bot/src/client.ts"
export const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1" });
```
````

TypeScript, TSX, JavaScript, JSON, Bash (`sh`, `shell`, `console`), Python, YAML und diff werden hervorgehoben; andere Sprachen erscheinen als einfacher Text. Rohes HTML wird nicht gerendert: Es erscheint als Text. Codeblöcke enthalten nie Kommentare: Einen Befehl im Satz davor erklären.

## Schreiben

- Jede Aussage beschreibt Kaeso so, wie sein Code es tut: Einstellungen gegen die Konfiguration des Gateways prüfen, Beschriftungen gegen den englischen Katalog der Konsole und Verhalten gegen Gateway, KPP und Konsole, bevor es geschrieben wird. Nie eine Funktion dokumentieren, die es nicht gibt.
- Seiten beschreiben den aktuellen Stand. Pläne, offene Fragen und To-do-Listen gehören in Issues.
- Satzschreibung für Titel und Überschriften, einfache Worte, kurze Sätze, keine Ausrufezeichen.
- Beschriftungen der Konsole sind fett und genau so geschrieben, wie die Konsole sie zeigt: **API-Schlüssel**, **Schlüssel erstellen**, **Einstellungen** › **Instanz**.
- Beispiele nutzen Kaesos eigene Namen und Daten (`kaeso.example.com`, `sk-kso-…`, `owner@example.com`), nie echte Schlüssel oder personenbezogene Daten.

## Übersetzen

Jede Seite gibt es auf Englisch, Deutsch und vereinfachtem Chinesisch. Eine Änderung an einer englischen Seite wird im selben Pull Request in `content/de` und `content/zh-CN` vorgenommen.

- **Gleiche Struktur.** Derselbe Dateiname, dasselbe `order`, dieselben Überschriften in derselben Reihenfolge und dieselben Codeblöcke, Byte für Byte. Titel, Beschreibung, Text, Tabellen und Hinweisboxen übersetzen.
- **Links.** Die Datei des Links beibehalten; sein Anker ist der Anker der Überschrift auf der übersetzten Seite selbst.
- **Was bleibt.** Code, Befehle, Einstellungen (`KAESO_*`), Pfade, Feldnamen, Rechte (`keys.all.view`), Produktnamen und Modell-IDs.
- **Beschriftungen der Konsole.** Die Übersetzung der Konsole für jede Beschriftung nutzen, aus ihren Katalogen in `apps/console/src/i18n/locales/de` und `apps/console/src/i18n/locales/zh-CN`.
- **Natürlich klingen**, nicht Wort für Wort, und jede Tatsache und jede Zahl behalten.

Der Hausstil und das Glossar stehen unter [Übersetzungen](translations.md#hausstil). Zwei Regeln gelten besonders für Seiten:

| | Deutsch | Vereinfachtes Chinesisch |
| --- | --- | --- |
| Schritte | Infinitiv, Verb am Ende: „Schlüssel erstellen“ | Verb zuerst: „创建密钥“ |
| Beschreibungen | Ein Satz | Ein Satz, endet auf „。“ |

Die eigenen Worte der Website, etwa „Auf GitHub bearbeiten“ oder die Suche, stehen in `apps/docs/src/i18n/messages/en.ts`, `de.ts` und `zh-CN.ts`, typisiert, sodass eine fehlende Übersetzung den Typecheck fehlschlagen lässt.
