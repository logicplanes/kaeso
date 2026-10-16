---
title: E-Mails
description: Wie das Gateway die E-Mails baut, die es sendet, für Mitwirkende an seinem Crate kaeso-mail.
order: 4
---

Jede E-Mail, die Kaeso sendet, kommt aus dem Crate `crates/kaeso-mail` des Gateways: ein Layout, ein Stylesheet, gebaut aus Kaesos Tokens, und eine Funktion pro E-Mail, die festlegt, was diese E-Mail sagt, auf Englisch, Deutsch und vereinfachtem Chinesisch. Was Betreiber einrichten und sehen, steht unter [E-Mail](../../self-hosting/email.md); diese Seite beschreibt das Design und die Gründe dahinter.

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

- **Keine Karte.** Eine einzige linksbündige Spalte, höchstens 562px breit, auf der Fläche, wie in der Konsole: weiß im hellen Modus, schwarz im dunklen. Kein Panel, kein Rahmen, keine graue Seite drumherum.
- **Text.** Ein kurzer Betreff (höchstens 60 Zeichen, kein Punkt), eine Überschrift (höchstens 48), höchstens ein Satz, höchstens ein Button und höchstens eine leise Zeile, etwa der Ablauf oder was zu tun ist, wenn man es nicht selbst war. Sätze folgen den Textregeln des Gateways: ein Satz, ein Semikolon für einen zweiten Gedanken, ein Punkt am Ende, höchstens 100 Zeichen. Ein Test erzwingt das.
- **Keine rohen Links.** Das HTML zeigt nie eine URL: Der Button trägt sie. Der Klartextteil hat den vollständigen Link in einer eigenen Zeile; das lesen reine Text-Clients, Tests und die End-to-End-Suite der Konsole.
- **Fußzeile.** „Gesendet von Kaeso unter“ und der Host der Installation, aus `KAESO_PUBLIC_URL` und mit ihm verlinkt, und bei Hinweisen, die immer gesendet werden, der Grund: Sicherheits- und Organisationshinweise sagen das, Warnungen sagen, wie sie sich abschalten lassen.
- **Klartext** sagt dieselben Worte in derselben Reihenfolge und endet mit einer Signaturzeile `-- `.

Der gerenderte Text und das HTML jeder E-Mail liegen als Snapshots in `crates/kaeso-mail/src/snapshots/`.

## Wie sie gebaut wird

| Datei | Aufgabe |
| --- | --- |
| `templates.rs` | Eine Funktion pro E-Mail ([was Kaeso sendet](../../self-hosting/email.md#was-kaeso-sendet)), die ihr Katalog ist: Betreff, Überschrift, Satz, Button, leise Zeile und Versandgrund, in jeder Sprache. `Email::render` rendert sie für eine Installation. |
| `locale.rs` | `Locale` (`en`, `de`, `zh-CN`), wie eine gewählt wird, und Datumsangaben und Zahlen so, wie jede Sprache sie schreibt. |
| `layout.rs` | Das gemeinsame Layout als HTML (maud: zur Compile-Zeit geprüft, jeder Wert escapet) und als Klartext. |
| `style.rs` | Die Werte aus `@logicplanes/kaeso-tokens`, einmal im Makro `token!` festgehalten, der Inline-Stil jeder Rolle, zur Compile-Zeit daraus zusammengesetzt, und das Stylesheet. Das Layout schreibt nie eine eigene Farbe oder Größe. |
| `assets.rs`, `assets/` | Die Bilder der Wortmarke und ihre SVG-Quellen. |
| `mailer.rs` | Die Warteschlange für bis zu 1.024 E-Mails, drei Wiederholungen nach 1, 5 und 30 Sekunden, die Transporte SMTP, Log und Arbeitsspeicher und die MIME-Struktur. |

**E-Mail-taugliches HTML.** Tabellen für die Struktur (`role="presentation"`), Inline-Stile für alles, was inline sein kann, und ein `<style>`-Block nur für das, was es nicht kann: Farbschemata, Resets und Dunkelmodus. Eine feste Spalte von 562px für Outlook für Windows, das `max-width` ignoriert, in bedingten Kommentaren, mit Segoe UI als Schrift und Bildern mit 96 DPI. Keine Webfonts, kein externes CSS und keine externen Bilder, keine Skripte, keine Tracking-Pixel. Der Font-Stack ist der der Konsole ohne ihre Webfonts: Inter, wo installiert, dann die Oberflächenschrift jeder Plattform.

**Der Button** ist bulletproof: ein gepolsterter Link mit 9999px Radius (eine `xl`-Pille: 48px hoch, 22px Innenabstand, 15px medium) überall, und ein VML-`roundrect` mit 50 % Bogen für Outlook für Windows, das einen Link weder polstern noch abrunden kann. VML braucht eine feste Breite, aus der Beschriftung mit 9px pro Zeichen (16px pro chinesischem) berechnet, sodass sie nie umbricht.

**Versteckter Vorschautext.** Der Satz, gefolgt von unsichtbaren Zeichen, sodass Posteingangslisten ihn nach dem Betreff zeigen statt der ersten Wörter des Layouts.

**Escaping.** Namen, die Personen wählen, etwa von Organisationen, Einladenden, Schlüsseln und Projekten, escapet maud in Text und Attributen. Das einzige nicht escapte Markup ist fest: der Doctype, die bedingten Kommentare für Outlook und der VML-Button, der die eingebettete URL und Beschriftung escapet. Tests schicken Markup und Anführungszeichen durch jeden Namen.

**MIME.** `multipart/alternative` mit dem Klartext zuerst und einem HTML-Teil `multipart/related`, der beide PNGs der Wortmarke als Inline-Teile per `Content-ID` trägt, ohne Dateinamen, sodass Mail-Apps sie nicht als Anhänge auflisten.

## Sprachen

Jede E-Mail gibt es auf Englisch, Deutsch und vereinfachtem Chinesisch; das Design ist in jeder gleich, nur die Worte ändern sich, und `lang` im HTML sagt, welche. Englisch ist die Quelle.

**Welche Sprache.** `services::mail_lang` in `kaeso-server` entscheidet, aus der Wahl der Person in den Kontoeinstellungen (`users.locale`, wobei `null` „Automatisch“ ist) und dem `Accept-Language` der Anfrage, die die E-Mail ausgelöst hat: die erste Sprache darin, in der Kaeso schreibt, nach Präferenz, sodass `de-AT` Deutsch ist, jedes Chinesisch vereinfachtes Chinesisch und jedes Englisch Englisch. Das Nachschlagen der Wahl hält nie eine E-Mail auf: Kann die Datenbank nicht antworten, entscheiden die Rückfallregeln. Die Regeln für jede Art von Empfänger stehen unter [E-Mail](../../self-hosting/email.md#sprachen).

**Schreiben.** Die Funktion jeder E-Mail hat ein `match` auf die Sprache mit ganzen Sätzen und namentlich eingesetzten Werten, nie zusammengefügte Fragmente. Glossar und Hausstil sind die [Schreibregeln von `@logicplanes/kaeso-ui`](https://github.com/logicplanes/kaeso/blob/dev/packages/ui/foundations/writing.md). E-Mails sprechen Personen förmlich an: Deutsch nutzt „Sie“ in kurzen Sätzen; Chinesisch ist knapp, nutzt „您“, Satzzeichen in voller Breite und ein Leerzeichen zwischen chinesischen und lateinischen Wörtern oder Ziffern, das `locale::zh` an den Nahtstellen setzt, nie innerhalb eines Namens. Deutsch und Chinesisch halten die Textregeln mit dem Spielraum der Konsole von 15 % ein: Betreff 69, Überschrift 55, Satz 115 Zeichen. Ein Test prüft sie, die Tags des Layouts gegen Englisch und Betreff und Überschrift jeder E-Mail in jeder Sprache.

**Datumsangaben und Zahlen** folgen der Sprache: `2026-10-14` auf Englisch, `14. Okt. 2026` auf Deutsch, `2026年10月14日` auf Chinesisch; Beträge `1250`, `1.250`, `1,250`; Prozente `80%`, `80 %`, `80%`.

## Die Wortmarke

Oberflächen zeigen die Wortmarke allein, oben links, in 60×12 (`Logo.Wordmark size="nav"`), und E-Mails ebenso: Das Bild ist 64×16 mit 2px Freiraum, bei 26px platziert, sodass die Glyphen bei 28px beginnen, auf derselben Linie wie der Text darunter.

**Inline mitgeliefert, nicht verlinkt.** Die öffentliche URL einer selbst gehosteten Installation kann intern oder unbekannt sein, sodass ein verlinktes Bild womöglich nicht lädt, und viele Mail-Apps blockieren entfernte Bilder standardmäßig; Inline-Teile erscheinen immer und brauchen nichts von der Installation. Die beiden PNGs zusammen sind etwa 6 KB groß.

**Scharf auf jedem Bildschirm.** Jedes PNG hat die dreifache Anzeigegröße (192×48), gerendert aus `assets/wordmark-light.svg` und `assets/wordmark-dark.svg`, deren Pfade die aus `@logicplanes/kaeso-ui` sind. Ein Test rendert die SVGs mit resvg neu und schlägt fehl, wenn die committeten PNGs abweichen; `KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets` schreibt sie neu. Beide haben `alt="Kaeso"` und die Attribute `width` und `height`.

## Dunkelmodus

Kaeso steuert den Dunkelmodus selbst, statt ihn der automatischen Invertierung jeder Mail-App zu überlassen. Drei Arten von Mail-Apps, drei Ebenen:

1. **Apps, die `prefers-color-scheme` folgen**, etwa Apple Mail auf macOS und iOS, Outlook für Mac, iOS und Android, Thunderbird, Samsung Email und Hey. Das Dokument deklariert beide Schemata (`<meta name="color-scheme" content="light dark">`, `supported-color-schemes` und `:root { color-scheme: light dark }`), was ihnen sagt, es nicht zu invertieren, und die Regeln `@media (prefers-color-scheme: dark)` des Stylesheets schalten jede Rolle mit `!important` auf ihr dunkles Token um, da Inline-Stile sonst gewinnen: die schwarze Fläche, Text in `#fafafa`, sekundärer und tertiärer Text in `#a3a3a3` und `#858585`, der Button auf `#fafafa` mit schwarzem Text und die helle Wortmarke gegen die dunkle getauscht.
2. **Outlook.com und das neue Outlook**, die E-Mails selbst abdunkeln und die Elemente, die sie umfärben, mit `data-ogsc` (Farbe) und `data-ogsb` (Hintergrund) markieren. Dieselben Regeln, unter `[data-ogsc]` und `[data-ogsb]`, ersetzen deren Schätzungen durch Kaesos dunkle Werte, sodass das Ergebnis dasselbe ist wie auf der ersten Ebene.
3. **Apps, die ungefragt invertieren**, etwa die Gmail-Apps, Outlook für Windows im Dunkelmodus und Gmail mit Konten anderer Anbieter, das `<style>` verwirft. Dort lässt sich nichts umschalten, daher ist das helle Design so gebaut, dass es die Invertierung übersteht:
   - Text ist fast schwarz `#0a0a0a`, nie reines Schwarz, und die Tinte des Buttons ist `#0a0a0a` mit weißem Text, sodass er invertiert zu einer hellen Pille mit dunklem Text wird, dem eigenen Button des dunklen Themes;
   - die helle Wortmarke hat einen weißen Halo von 1,25px, unsichtbar auf Weiß; wo der Hintergrund dahinter abgedunkelt wird und das Bild nicht, umrandet der Halo die Glyphen und hält die Wortmarke lesbar;
   - die dunkle Wortmarke ist mit `display:none`, `max-height:0` und `mso-hide:all` in einem bedingten Kommentar versteckt, sodass sie nie allein erscheint.

Das Stylesheet wird aus demselben Token-Makro generiert wie die Inline-Stile, und Tests prüfen, dass jede dunkle Regel `!important` ist und dass jede Rolle für jede Ebene eine Regel hat.

## Vorschau

Der [Entwicklungs-Stack](../local-setup.md#3-entwicklungs-stack-starten) enthält Mailpit, dessen Weboberfläche unter http://127.0.0.1:58025 eine HTML-Kompatibilitätsprüfung und eine dunkle Vorschau bietet. `cargo test -p kaeso-mail --test smtp` sendet jede E-Mail in jeder Sprache dorthin und prüft, dass jede mit beiden Teilen und beiden Bildern inline ankommt und dass die HTML-Prüfung von Mailpit, auf Basis von caniemail-Daten, gewichtet nach dem Anteil jeder Mail-App, mindestens 88 % Unterstützung meldet; danach löscht der Test seine Nachrichten. `KAESO_TEST_SMTP_URL` und `KAESO_TEST_MAIL_API` verweisen ihn anderswohin.

Jede E-Mail erreicht etwa 91 % unterstützt, 8 % teilweise und 1 % nicht unterstützt. Die teilweisen und nicht unterstützten Einträge sind die erwarteten: `@media`, `<style>` und `!important` für den Dunkelmodus, die Mail-Apps ohne sie durch die dritte Ebene ersetzen; `border-radius`, das Outlook für Windows durch VML ersetzt; und Stile auf `<body>`, die die Wrapper wiederholen.
