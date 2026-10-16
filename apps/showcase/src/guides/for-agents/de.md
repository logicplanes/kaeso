---
title: Für Agents
description: Wie ein Agent mit dieser Bibliothek Kaeso-Oberflächen baut. Vor dem Schreiben von Oberflächencode lesen.
---

# Kaeso UI für Agents

Wie ein Agent Kaeso-Oberflächen mit `@logicplanes/kaeso-ui` baut. Vor dem Schreiben von Oberflächencode lesen. Die Datei liegt im Paket (`node_modules/@logicplanes/kaeso-ui/AGENTS.md`) und ist die Seite [ui.kaeso.app/for-agents](https://ui.kaeso.app/for-agents).

## Im eigenen Projekt laden

Damit der Agent diese Regeln bei jeder Arbeit an der Oberfläche liest:

- **Claude Code:** diese Zeile in die `CLAUDE.md` des Projekts aufnehmen:

  ```
  @node_modules/@logicplanes/kaeso-ui/AGENTS.md
  ```

- **Codex, Cursor und andere Agents, die `AGENTS.md` lesen:** diese Zeile in die `AGENTS.md` des Projekts aufnehmen:

  ```
  Before writing interface code, read node_modules/@logicplanes/kaeso-ui/AGENTS.md and follow it.
  ```

## Grundlagen

Das Designsystem hinter den Komponenten, in `node_modules/@logicplanes/kaeso-ui/foundations/`. Die Datei lesen, die zur Aufgabe passt:

| Datei                          | Lesen, wenn                                                                 |
| ------------------------------ | --------------------------------------------------------------------------- |
| `foundations/tokens.md`        | eine Klasse geschrieben wird: Es gibt nur Token-Werte                       |
| `foundations/color.md`         | eine Fläche, eine Textrolle oder eine Statusfarbe gewählt wird              |
| `foundations/typography.md`    | Text, Zahlen, IDs oder Code gesetzt werden                                  |
| `foundations/shape.md`         | ein Radius gewählt oder Flächen verschachtelt werden                        |
| `foundations/elevation.md`     | eine Linie, ein Ring oder eine erhabene Fläche gezeichnet wird              |
| `foundations/motion.md`        | etwas animiert oder ein Ladezustand gezeigt wird                            |
| `foundations/icons.md`         | ein Symbol platziert wird                                                   |
| `foundations/writing.md`       | eine Beschriftung, Meldung, ein leerer Zustand oder Fehler geschrieben wird |
| `foundations/accessibility.md` | etwas Interaktives gebaut wird                                              |
| `foundations/psychology.md`    | eine Ansicht geprüft wird: die 27 Wahrnehmungsregeln                        |

## Maschinenlesbar

- `node_modules/@logicplanes/kaeso-ui/ai/component-registry.md`: jede Komponente mit Import, Props, Typen, Standardwerten und Varianten, aus den Typen erzeugt. Dasselbe als JSON in `ai/component-registry.json`.
- `node_modules/@logicplanes/kaeso-ui/ai/component-guides.md`: für jede Komponente, wofür sie da ist, Tastatur, Barrierefreiheit, Do und Don't und der Code jedes Beispiels. Den Abschnitt einer Komponente vor ihrer ersten Verwendung lesen.
- `node_modules/@logicplanes/kaeso-ui/ai/tokens.md`: jeder Token-Wert, hell und dunkel. `ai/icons.md`: das Symbolvokabular.
- [ui.kaeso.app/llms.txt](https://ui.kaeso.app/llms.txt): diese Regeln und jede Komponente als reiner Text.
- [ui.kaeso.app/components.json](https://ui.kaeso.app/components.json): die Registry online.
- Jede Komponentenseite auf ui.kaeso.app hat **Seite kopieren**, das sie als Markdown kopiert.

## Einrichtung

Mit Tailwind CSS v4 im Stylesheet der App:

```css
@import "tailwindcss";
@import "@logicplanes/kaeso-ui/tailwind.css";
```

Ohne Tailwind `@logicplanes/kaeso-ui/styles.css` einmal importieren. Dann die App einmal umschließen:

```tsx
import { ThemeProvider, TooltipProvider, Toaster } from "@logicplanes/kaeso-ui";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider>
      <TooltipProvider>
        {children}
        <Toaster />
      </TooltipProvider>
    </ThemeProvider>
  );
}
```

## Die Regeln

1. **Zusammensetzen, nie gestalten.** Die Konsole besteht nur aus `@logicplanes/kaeso-ui`. Fehlt etwas, kommt es in die Bibliothek und wird nicht in der App gezeichnet.
2. **`className` ist nur für das Layout**: Außenabstand, Breite, Rasterplatzierung, Flex. Nie Farbe, Radius, Schatten, Schrift oder Innenabstand an einer Komponente; dafür ihre Props und Varianten verwenden.
3. **Es gibt nur Token-Werte.** `bg-panel`, `text-fg-secondary`, `h-36`, `rounded-pill`. Es gibt kein `bg-blue-500`, kein `p-7`, keine beliebigen Werte, kein `dark:`.
4. **Eingabefelder in `Field` einschließen** (oder ihnen `label` geben) für Beschriftung, Hinweis und Fehler. Ein Eingabefeld ohne Beschriftung oder `aria-label` ist ein Fehler.
5. **Ein primärer Button pro Ansicht.** Alles andere ist secondary oder ghost.
6. **Keine dekorativen Karten und keine dekorativen Symbole.** Inhalt sitzt auf dem Panel; leere Zustände und Dialoge tragen Worte.
7. **Farbe nur für Daten und Status**, und ein Status hat immer ein Wort.
8. **Die Formatierer verwenden** aus `@logicplanes/kaeso-ui/format` für Geld, Tokens, Dauern und Daten. Geld bleibt eine exakte Dezimalzeichenkette.
9. **Das Symbolvokabular verwenden**: `icons.key`, `icons.model`, nie ein Symbol, das nach Aussehen gewählt wurde.
10. **Texte folgen der Stimme**: Satzanfang groß, auf Buttons zuerst das Verb, keine Ausrufezeichen oder Emoji, Fehler, die sagen, was als Nächstes zu tun ist.
11. **Raum lassen.** Das Panel atmen lassen: Die Komponenten bringen ihre eigenen Abstände mit, also Ansichten nicht mit zusätzlichen Hüllen, Rahmen oder Überschriften füllen. Weniger Elemente, jedes klar.

## Welche Komponente für welche Aufgabe

| Aufgabe                                          | Verwenden                                                           |
| ------------------------------------------------ | ------------------------------------------------------------------- |
| App-Layout                                       | `AppFrame` mit `Sidebar`, `PageHeader`, `AppFrame.Body`             |
| Einstellungsseite                                | `AppFrame.Settings`, `SettingsGroup`, `SettingsRow`                 |
| Eine Sammlung, die man überfliegt und bearbeitet | `List` mit Gruppenköpfen                                            |
| Eine Sammlung, die man spaltenweise vergleicht   | `DataTable` (virtualisiert, sortierbar)                             |
| Filter über einer Sammlung                       | `Toolbar` mit `FilterChip`, `SearchInput`, `DisplayOptions`         |
| Eine Wahl zwischen 2–4 Ansichten                 | `Segmented`; Seitenabschnitte: `PillTabs`; Detailabschnitte: `Tabs` |
| Eine von vielen Optionen                         | `Select`; durchsuchbar: `Combobox`                                  |
| Eine Bestätigung einer unumkehrbaren Aktion      | `AlertDialog`; Löschen: der Block `DeleteResource`                  |
| Eine Aufgabe in mehreren Schritten               | `StepDialog`                                                        |
| Rückmeldung nach einer Aktion                    | `toast()`, mit Rückgängig für umkehrbare Aktionen                   |
| Ein Geheimnis, das einmal gezeigt wird           | `SecretReveal`                                                      |
| Zahlen auf einen Blick                           | `MetricRow` mit `Stat`; im Zeitverlauf: `TimeseriesChart`           |
| Noch nichts zu zeigen                            | `Empty` mit einer Aktion                                            |

## Eine Ansicht

```tsx
import { AppFrame, Sidebar, PageHeader, Toolbar, FilterChip, SearchInput, DataTable, Button, icons } from "@logicplanes/kaeso-ui";

export function RequestLogs({ rows }: { rows: LogRow[] }) {
  return (
    <AppFrame brandHref="/" sidebar={<ConsoleSidebar current="logs" />}>
      <PageHeader start={<AppFrame.MenuButton />} title="Request logs" actions={<Button variant="secondary">Export CSV</Button>} />
      <Toolbar>
        <FilterChip property="Status" operator="is" value="2xx" />
        <SearchInput aria-label="Search logs" placeholder="Search logs" />
      </Toolbar>
      <AppFrame.Body>
        <DataTable data={rows} columns={columns} />
      </AppFrame.Body>
    </AppFrame>
  );
}
```
