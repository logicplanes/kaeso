---
title: E-Mails
description: Wie das Gateway die E-Mails baut, die es sendet, für Änderungen am Crate kaeso-mail.
order: 4
---

Jede E-Mail, die Kaeso sendet, stammt aus `crates/kaeso-mail`: ein Layout, ein aus den Tokens von Kaeso gebautes Stylesheet und eine Funktion pro E-Mail auf Englisch, Deutsch und vereinfachtem Chinesisch. [E-Mail](../../self-hosting/email.md) behandelt, was Betreiber einrichten und sehen; diese Seite behandelt den Code.

## Was eine E-Mail ist

| Teil | In der Anmelde-E-Mail | Stil |
| --- | --- | --- |
| Wortmarke | Kaeso | Allein, oben links, 28 px nach innen und 28 px nach unten |
| Überschrift | Sign in to Kaeso | `title-lg` |
| Ein Satz | The link works once and expires in 15 minutes. | `body-lg`, sekundär |
| Ein Button | Sign in | Die Pille `xl`, Tinte auf Weiß |
| Eine leise Zeile | If you did not ask to sign in, you can ignore this email. | `small`, tertiär |
| Fußzeile | Sent by Kaeso at kaeso.example.com. | `caption` |

- **Keine Karte.** Eine einzige linksbündige Spalte, höchstens 562 px breit, auf der Fläche: weiß im hellen Modus, schwarz im dunklen.
- **Längen.** Ein Betreff von höchstens 60 Zeichen ohne Punkt, eine Überschrift von höchstens 48 und Sätze von höchstens 100. Deutsch und Chinesisch bekommen 15 % mehr: 69, 55 und 115. Ein Test setzt die Grenzen durch.
- **Keine nackten Links.** Das HTML zeigt nie eine URL; der Button trägt sie. Der Textteil enthält den vollständigen Link auf einer eigenen Zeile und sagt dieselben Worte in derselben Reihenfolge.
- **Fußzeile.** „Sent by Kaeso at“ und der Host der Installation, aus `KAESO_PUBLIC_URL`. Hinweise sagen außerdem, warum sie ankamen: Sicherheits- und Organisationshinweise werden immer gesendet, und Warnungen sagen, wie man sie abschaltet.

Text und HTML jeder E-Mail sind Snapshots in `crates/kaeso-mail/src/snapshots/`.

## Wie sie gebaut wird

| Datei | Aufgabe |
| --- | --- |
| `templates.rs` | Eine Funktion pro E-Mail, der Katalog dessen, [was Kaeso sendet](../../self-hosting/email.md#was-kaeso-sendet): Betreff, Überschrift, Satz, Button, leise Zeile und warum sie gesendet wird, in jeder Sprache |
| `locale.rs` | `Locale` (`en`, `de`, `zh-CN`) sowie Datumsangaben und Zahlen, wie jede Sprache sie schreibt |
| `layout.rs` | Das gemeinsame Layout, als HTML mit maud und als reiner Text |
| `style.rs` | Die Werte von `@logicplanes/kaeso-tokens` in seinem Makro `token!`, der Inline-Stil jeder Rolle und das Stylesheet. Das Layout schreibt nie eine eigene Farbe oder Größe |
| `assets.rs`, `assets/` | Die Bilder der Wortmarke und ihre SVG-Quellen |
| `mailer.rs` | Eine Warteschlange für bis zu 1.024 E-Mails, drei Wiederholungen nach 1, 5 und 30 Sekunden, die Transporte SMTP, Log und Speicher sowie die MIME-Struktur |

- **E-Mail-taugliches HTML.** Tabellen geben die Struktur vor, und alles, was ein Inline-Stil sein kann, ist einer. Ein `<style>`-Block enthält nur Farbschemata, Resets und den Dunkelmodus. Bedingte Kommentare geben Outlook für Windows eine feste Spalte von 562 px. Keine Webfonts, externen Bilder, Skripte oder Tracking-Pixel.
- **Der Button** ist ein Link mit Innenabstand und vollem Radius. Für Outlook für Windows, das einen Link weder auffüllen noch runden kann, ist er ein VML-`roundrect`, dessen Breite aus der Beschriftung kommt, sodass er nie umbricht.
- **Escaping.** maud escapet die Namen, die Menschen wählen, etwa Organisationen, Schlüssel und Projekte, in Text und Attributen. Das einzige nicht escapete Markup ist fest: der Doctype, die bedingten Kommentare für Outlook und der VML-Button, der escapet, was er einbettet. Tests schicken Markup und Anführungszeichen durch jeden Namen.
- **MIME.** `multipart/alternative` mit dem Text zuerst und einem HTML-Teil, der beide PNGs der Wortmarke per `Content-ID` eingebettet trägt, sodass Mail-Apps sie nicht als Anhänge aufführen.

## Sprachen

`services::mail_lang` in `kaeso-server` wählt die Sprache. Die Wahl der Person in ihren Kontoeinstellungen gewinnt (`users.locale`; `null` heißt Automatisch). Sonst nimmt es die erste Sprache, die Kaeso schreibt, aus dem `Accept-Language` der Anfrage, die die E-Mail ausgelöst hat. Kann die Datenbank nicht antworten, entscheiden die Rückfallregeln, sodass die Abfrage nie eine E-Mail aufhält. [E-Mail](../../self-hosting/email.md#sprachen) nennt die Regeln für jede Art von Empfänger.

Die Funktion jeder E-Mail hat ein `match` auf die Sprache, mit ganzen Sätzen und namentlich eingesetzten Werten, nie zusammengesetzten Bruchstücken. Deutsch spricht Menschen mit „Sie“ an. Chinesisch nutzt „您“, Satzzeichen in voller Breite und ein Leerzeichen zwischen chinesischen und lateinischen Wörtern oder Ziffern, das `locale::zh` an den Nahtstellen setzt. Datumsangaben und Zahlen folgen der Sprache:

| | Englisch | Deutsch | Chinesisch |
| --- | --- | --- | --- |
| Datum | `2026-10-14` | `14. Okt. 2026` | `2026年10月14日` |
| Betrag | `1250` | `1.250` | `1,250` |
| Prozent | `80%` | `80 %` | `80%` |

## Die Wortmarke

E-Mails tragen die Wortmarke als zwei eingebettete PNGs, hell und dunkel, zusammen etwa 6 KB. Die Adresse einer selbst betriebenen Installation kann intern sein, und viele Mail-Apps blockieren externe Bilder, verlinkte Bilder könnten also fehlen. Jedes PNG hat die dreifache Anzeigegröße und wird aus `assets/wordmark-light.svg` und `assets/wordmark-dark.svg` gerendert. Ein Test rendert die SVGs erneut mit resvg und schlägt fehl, wenn die committeten PNGs abweichen. So schreiben Sie sie neu:

```bash
KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets
```

## Dunkelmodus

Kaeso legt den Dunkelmodus selbst fest, statt ihn der Invertierung jeder Mail-App zu überlassen. Mail-Apps fallen in drei Arten, und jede bekommt eine eigene Ebene:

1. **Apps, die `prefers-color-scheme` folgen**, etwa Apple Mail und Thunderbird. Das Dokument erklärt beide Farbschemata, sodass sie es nicht invertieren, und `@media (prefers-color-scheme: dark)` schaltet jede Rolle mit `!important` auf ihr dunkles Token.
2. **Outlook.com und das neue Outlook**, die E-Mails selbst abdunkeln und das Umgefärbte mit `data-ogsc` und `data-ogsb` markieren. Dieselben Regeln unter diesen Attributen ersetzen ihre Schätzung durch die dunklen Werte von Kaeso.
3. **Apps, die ungefragt invertieren**, etwa die Gmail-Apps. Das helle Design übersteht die Invertierung: fast schwarzer Text, ein Button, der zur hellen Pille des dunklen Themes wird, ein weißer Rand um die helle Wortmarke und eine dunkle Wortmarke, die verborgen bleibt.

Das Stylesheet kommt aus demselben Token-Makro wie die Inline-Stile. Tests prüfen, dass jede dunkle Regel `!important` ist und jede Rolle in jeder Ebene eine Regel hat.

## E-Mails ansehen

Der [Entwicklungs-Stack](../stack.md) enthält Mailpit, dessen Weboberfläche unter http://127.0.0.1:58025 eine HTML-Kompatibilitätsprüfung und eine dunkle Vorschau bietet. Senden Sie jede E-Mail in jeder Sprache dorthin:

```bash
cargo test -p kaeso-mail --test smtp
```

Der Test prüft, dass jede E-Mail mit beiden Teilen und beiden eingebetteten Bildern ankommt und dass die HTML-Prüfung von Mailpit genug Unterstützung über die Mail-Apps meldet. Danach löscht er seine Nachrichten. `KAESO_TEST_SMTP_URL` und `KAESO_TEST_MAIL_API` lenken ihn anderswohin.
