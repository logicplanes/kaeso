---
title: E-Mails
description: Wie das Gateway die E-Mails baut, die es sendet, für Mitwirkende an seinem Crate kaeso-mail.
order: 4
---

Jede E-Mail, die Kaeso sendet, stammt aus `crates/kaeso-mail` im Gateway: ein Layout, ein Stylesheet aus den Tokens von Kaeso und eine Funktion pro E-Mail, die festlegt, was diese E-Mail sagt, auf Englisch, Deutsch und vereinfachtem Chinesisch. [E-Mail](../../self-hosting/email.md) beschreibt, was Betreiber einrichten und sehen; diese Seite beschreibt das Design und die Gründe dafür.

## Was eine E-Mail ist

```text
KAESO                                   the wordmark alone, top left, 28px in and 28px down

Sign in to Kaeso                        heading: title-lg, 28/34, semibold
The link works once and expires in      one sentence: body-lg, 15/22, secondary
15 minutes.

( Sign in )                             one button: the xl pill, ink on white

If you did not ask to sign in, you      one quiet line: small, 13/18, tertiary
can ignore this email.

Sent by Kaeso at kaeso.example.com.     footer: caption, 12/16; the install, and why
```

- **Keine Karte.** Eine einzige linksbündige Spalte, höchstens 562px breit, auf der Arbeitsfläche, wie in der Konsole: weiß im hellen Modus, schwarz im dunklen. Kein Panel, kein Rahmen, keine graue Seite drumherum.
- **Text.** Ein Betreff mit höchstens 60 Zeichen ohne Punkt, eine Überschrift mit höchstens 48, dann höchstens ein Satz, ein Button und eine leise Zeile, etwa der Ablauf eines Links oder was zu tun ist, wenn man es nicht selbst war. Sätze folgen den Textregeln des Gateways: ein Satz, ein Semikolon für einen zweiten Gedanken, ein Punkt am Ende, höchstens 100 Zeichen. Ein Test erzwingt die Grenzen.
- **Keine nackten Links.** Das HTML zeigt nie eine URL; der Button trägt sie. Der Klartextteil enthält den Link vollständig in einer eigenen Zeile, und genau das lesen reine Text-Clients, Tests und die End-to-End-Suite der Konsole.
- **Fußzeile.** „Gesendet von Kaeso unter“ und der Host der Installation, aus `KAESO_PUBLIC_URL` und darauf verlinkt, und bei Benachrichtigungen, warum sie kam: Sicherheits- und Organisationshinweise sagen, dass sie immer gesendet werden, und Warnungen sagen, wie man sie abschaltet.
- **Klartext** sagt dieselben Worte in derselben Reihenfolge und endet mit einer Signaturzeile `-- `.

Text und HTML jeder E-Mail sind als Snapshots in `crates/kaeso-mail/src/snapshots/` festgehalten.

## Wie sie gebaut wird

| Datei | Aufgabe |
| --- | --- |
| `templates.rs` | Eine Funktion pro E-Mail ([was Kaeso sendet](../../self-hosting/email.md#was-kaeso-sendet)), die zugleich ihr Katalog ist: Betreff, Überschrift, Satz, Button, leise Zeile und warum sie gesendet wird, in jeder Sprache. `Email::render` rendert sie für eine Installation. |
| `locale.rs` | `Locale` (`en`, `de`, `zh-CN`), wie eine Sprache gewählt wird, und Datumsangaben und Zahlen, wie jede Sprache sie schreibt. |
| `layout.rs` | Das gemeinsame Layout als HTML (maud: zur Compile-Zeit geprüft, jeder Wert escapt) und als Klartext. |
| `style.rs` | Die Werte von `@logicplanes/kaeso-tokens`, einmal in seinem Makro `token!` geschrieben, der Inline-Style jeder Rolle, zur Compile-Zeit daraus zusammengesetzt, und das Stylesheet. Das Layout schreibt nie eine eigene Farbe oder Größe. |
| `assets.rs`, `assets/` | Die Bilder der Wortmarke und ihre SVG-Quellen. |
| `mailer.rs` | Die Warteschlange für bis zu 1.024 E-Mails, drei Wiederholungen nach 1, 5 und 30 Sekunden, die Transporte SMTP, Log und Speicher sowie die MIME-Struktur. |

**E-Mail-taugliches HTML.** Tabellen geben die Struktur vor (`role="presentation"`), und alles, was ein Inline-Style sein kann, ist einer; ein `<style>`-Block enthält nur, was das nicht kann: Farbschemata, Resets und den Dunkelmodus. Outlook für Windows ignoriert `max-width`, daher geben ihm bedingte Kommentare eine feste Spalte von 562px, Segoe UI als Schrift und Bilder mit 96 DPI. Keine Webfonts, kein externes CSS und keine externen Bilder, keine Skripte, keine Tracking-Pixel. Der Schrift-Stack ist der der Konsole ohne ihre Webfonts: Inter, wo installiert, dann die Oberflächenschrift jeder Plattform.

**Der Button** funktioniert überall: ein Link mit Innenabstand und 9999px Radius (eine `xl`-Pille: 48px hoch, 22px Innenabstand, 15px medium) und für Outlook für Windows, das einen Link weder auffüllen noch abrunden kann, ein VML-`roundrect` mit 50 % Rundung. VML braucht eine feste Breite, berechnet aus der Beschriftung mit 9px pro Zeichen (16px pro chinesischem Zeichen), damit sie nie umbricht.

**Versteckter Vorschautext.** Der Satz, gefolgt von unsichtbaren Zeichen, damit Posteingänge ihn nach dem Betreff zeigen statt der ersten Wörter des Layouts.

**Escaping.** maud escapt die Namen, die Menschen wählen, etwa Organisationen, Einladende, Schlüssel und Projekte, in Text und Attributen. Das einzige nicht escapte Markup ist fest: der Doctype, die bedingten Kommentare für Outlook und der VML-Button, der die eingebettete URL und Beschriftung escapt. Tests schicken Markup und Anführungszeichen durch jeden Namen.

**MIME.** `multipart/alternative` mit dem Klartext zuerst und einem HTML-Teil `multipart/related`, der beide PNGs der Wortmarke als Inline-Teile per `Content-ID` trägt, ohne Dateinamen, damit Mail-Apps sie nicht als Anhänge auflisten.

## Sprachen

Jede E-Mail gibt es auf Englisch, Deutsch und vereinfachtem Chinesisch. Das Design ist in jeder Sprache gleich; nur die Worte ändern sich, und `lang` im HTML sagt, welche. Englisch ist die Quelle.

**Welche Sprache.** `services::mail_lang` in `kaeso-server` entscheidet anhand der Wahl der Person in den Kontoeinstellungen (`users.locale`, wobei `null` für Automatisch steht) und des `Accept-Language` der Anfrage, die die E-Mail ausgelöst hat: die erste Sprache darin, in der Kaeso schreibt, nach Präferenz, sodass `de-AT` Deutsch ist, jedes Chinesisch vereinfachtes Chinesisch und jedes Englisch Englisch. Das Nachschlagen der Wahl hält nie eine E-Mail auf: Kann die Datenbank nicht antworten, entscheiden die Rückfallregeln. [E-Mail](../../self-hosting/email.md#sprachen) nennt die Regeln für jede Art von Empfänger.

**Schreiben.** Die Funktion jeder E-Mail hat ein `match` auf die Sprache mit ganzen Sätzen und namentlich eingesetzten Werten, nie zusammengesetzten Bruchstücken. Glossar und Hausstil sind die [Schreibregeln von `@logicplanes/kaeso-ui`](https://github.com/logicplanes/kaeso/blob/dev/packages/ui/foundations/writing.md). E-Mails sprechen Personen förmlich an: Deutsch nutzt „Sie“ in kurzen Sätzen; Chinesisch ist knapp und nutzt „您“, Satzzeichen in voller Breite und ein Leerzeichen zwischen chinesischen und lateinischen Wörtern oder Ziffern, das `locale::zh` an den Nahtstellen setzt, nie innerhalb eines Namens.

Deutsch und Chinesisch halten die Textregeln mit dem Spielraum der Konsole von 15 % ein: Betreff 69, Überschrift 55 und Satz 115 Zeichen. Tests prüfen diese Grenzen, die Tags des Layouts gegen Englisch sowie Betreff und Überschrift jeder E-Mail in jeder Sprache.

**Datumsangaben und Zahlen** folgen der Sprache:

| | Englisch | Deutsch | Chinesisch |
| --- | --- | --- | --- |
| Datum | `2026-10-14` | `14. Okt. 2026` | `2026年10月14日` |
| Betrag | `1250` | `1.250` | `1,250` |
| Prozent | `80%` | `80 %` | `80%` |

## Die Wortmarke

Oberflächen zeigen die Wortmarke allein oben links in 60×12 (`Logo.Wordmark size="nav"`), und E-Mails ebenso: Das Bild ist 64×16 mit 2px Schutzraum, bei 26px platziert, sodass die Glyphen bei 28px beginnen, auf derselben Linie wie der Text darunter.

**Inline geliefert, nicht verlinkt.** Die öffentliche URL einer selbst gehosteten Installation kann intern oder unbekannt sein, sodass ein verlinktes Bild nicht laden könnte, und viele Mail-Apps blockieren externe Bilder standardmäßig. Inline-Teile werden immer angezeigt und brauchen nichts von der Installation. Die beiden PNGs zusammen sind etwa 6 KB groß.

**Scharf auf jedem Bildschirm.** Jedes PNG hat die dreifache Anzeigegröße (192×48), gerendert aus `assets/wordmark-light.svg` und `assets/wordmark-dark.svg`, deren Pfade die aus `@logicplanes/kaeso-ui` sind. Ein Test rendert die SVGs erneut mit resvg und schlägt fehl, wenn die eingecheckten PNGs abweichen; `KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets` schreibt sie neu. Beide Bilder haben `alt="Kaeso"` und die Attribute `width` und `height`.

## Dunkelmodus

Kaeso steuert den Dunkelmodus selbst, statt ihn der automatischen Invertierung jeder Mail-App zu überlassen. Mail-Apps fallen in drei Arten, und jede bekommt ihre eigene Ebene:

1. **Apps, die `prefers-color-scheme` folgen**, etwa Apple Mail auf macOS und iOS, Outlook für Mac, iOS und Android, Thunderbird, Samsung Email und Hey. Das Dokument erklärt beide Schemata (`<meta name="color-scheme" content="light dark">`, `supported-color-schemes` und `:root { color-scheme: light dark }`), was ihnen sagt, es nicht zu invertieren. Die Regeln `@media (prefers-color-scheme: dark)` des Stylesheets schalten jede Rolle mit `!important` auf ihr dunkles Token um, da Inline-Styles sonst gewinnen würden: die schwarze Arbeitsfläche, Text in `#fafafa`, sekundärer und tertiärer Text in `#a3a3a3` und `#858585`, der Button in `#fafafa` mit schwarzem Text und die helle Wortmarke statt der dunklen.
2. **Outlook.com und das neue Outlook**, die E-Mails selbst abdunkeln und die umgefärbten Elemente mit `data-ogsc` (Farbe) und `data-ogsb` (Hintergrund) markieren. Dieselben Regeln, unter `[data-ogsc]` und `[data-ogsb]`, ersetzen ihre Vermutungen durch die dunklen Werte von Kaeso, sodass das Ergebnis der ersten Ebene entspricht.
3. **Apps, die ungefragt invertieren**, etwa die Gmail-Apps, Outlook für Windows im Dunkelmodus und Gmail mit Konten anderer Anbieter, das `<style>` verwirft. Dort lässt sich nichts umschalten, daher ist das helle Design so gebaut, dass es die Invertierung übersteht:
   - Text ist fast schwarz, `#0a0a0a`, nie reines Schwarz, und die Farbe des Buttons ist `#0a0a0a` mit weißem Text, sodass er invertiert zu einer hellen Pille mit dunklem Text wird, dem Button des dunklen Themes;
   - die helle Wortmarke hat einen weißen Rand von 1,25px, auf Weiß unsichtbar; wo der Hintergrund dahinter abgedunkelt wird und das Bild nicht, umrandet der Rand die Glyphen und hält die Wortmarke lesbar;
   - die dunkle Wortmarke ist mit `display:none`, `max-height:0` und `mso-hide:all` in einem bedingten Kommentar versteckt, sodass sie nie allein erscheint.

Das Stylesheet wird aus demselben Token-Makro erzeugt wie die Inline-Styles, und Tests prüfen, dass jede dunkle Regel `!important` ist und jede Rolle in jeder Ebene eine Regel hat.

## Vorschau

Der [Entwicklungs-Stack](../local-setup.md#3-entwicklungs-stack-starten) enthält Mailpit, dessen Weboberfläche unter http://127.0.0.1:58025 eine HTML-Kompatibilitätsprüfung und eine dunkle Vorschau bietet. `cargo test -p kaeso-mail --test smtp` sendet jede E-Mail in jeder Sprache dorthin und prüft, dass:

- jede mit beiden Teilen und beiden Bildern inline ankommt;
- die HTML-Prüfung von Mailpit, gestützt auf Daten von caniemail und gewichtet nach dem Anteil jeder Mail-App, mindestens 88 % Unterstützung meldet.

Danach löscht der Test seine Nachrichten. `KAESO_TEST_SMTP_URL` und `KAESO_TEST_MAIL_API` richten ihn auf ein anderes Ziel.

Jede E-Mail erreicht etwa 91 % unterstützt, 8 % teilweise und 1 % nicht unterstützt. Die teilweise und nicht unterstützten Einträge sind erwartet: `@media`, `<style>` und `!important` für den Dunkelmodus, deren Fehlen die dritte Ebene auffängt; `border-radius`, das in Outlook für Windows VML ersetzt; und Styles auf `<body>`, die die Wrapper wiederholen.
