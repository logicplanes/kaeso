---
title: E-Mails
description: Wie das Gateway die E-Mails baut, die es sendet, für Änderungen am Crate kaeso-mail.
order: 4
---

Jede E-Mail, die Kaeso sendet, stammt aus `crates/kaeso-mail`: ein Layout, ein aus den Tokens von Kaeso gebautes Stylesheet und eine Funktion pro E-Mail auf Englisch, Deutsch und vereinfachtem Chinesisch. [E-Mail](../../self-hosting/email.md) behandelt, was Betreiber einrichten und sehen; diese Seite behandelt den Code.

## Was eine E-Mail ist

| Teil | In der Anmelde-E-Mail | Stil |
| --- | --- | --- |
| Wortmarke oder Titelbild | Kaeso | Die Wortmarke allein, oben links; Ankündigungen beginnen stattdessen mit einem [Titelbild](#titelbilder) |
| Wo, wenn eine Benachrichtigung eine Organisation oder ein Projekt betrifft | Acme / Checkout | `small`, tertiär, über der Überschrift |
| Überschrift | Bei Kaeso anmelden | `title-lg` |
| Ein Satz | Der Link funktioniert einmal und läuft in 15 Minuten ab. | `body-lg`, sekundär |
| Kennzahlen, wenn ein Bericht sie hat | 12.480 Anfragen, 312 Tool-Aufrufe, 41,20 $ | Eine Fläche mit bis zu drei Werten, `title` und `caption` |
| Details, wenn es Fakten gibt | Organisation: Acme, Anbieter: Anthropic, Schlüssel: Production (…8QAA) | Bezeichnung und Wert zwischen Haarlinien, `small` tertiär und `body-lg` |
| Eine Tabelle, wenn ein Bericht Projekte aufführt | Checkout, 9.120, 30,10 $ | Zeilen zwischen Haarlinien, Zahlen rechtsbündig in Tabellenziffern, bei mehreren Organisationen unter dem Namen jeder Organisation gruppiert |
| Eine Liste, wenn es Änderungen gibt | Was sich ändert, dann bis zu vier Zeilen der Release-Notizen; in der Zusammenfassung die Zeilen unter Acme oder Acme / Checkout | Eine kurze Bezeichnung und Zeilen mit Aufzählungspunkten |
| Ein Button | Anmelden | Die Pille `xl`, Tinte auf Weiß, 48 px hoch |
| Ein Textlink, wenn es mehr zu lesen gibt | Release-Notizen lesen | Unterstrichen, Tinte |
| Eine leise Zeile | Wenn Sie keine Anmeldung angefordert haben, können Sie diese E-Mail ignorieren. | `small`, tertiär |
| Fußzeile | Gesendet von Kaeso unter kaeso.example.com. | `caption` |

- **Keine Karte.** Eine einzige linksbündige Spalte, höchstens 560 px breit, auf der Fläche: weiß im hellen Modus, schwarz im dunklen. Die Abstände folgen dem 8-px-Raster, zwischen Gruppen mit mehr Raum als in ihnen; auf dem Telefon schrumpfen die Ränder auf 16 px und die Überschrift auf 24 px.
- **Längen.** Ein Betreff von höchstens 60 Zeichen ohne Punkt, eine Überschrift von höchstens 48 und Sätze von höchstens 100. Deutsch und Chinesisch bekommen 15 % mehr: 69, 55 und 115. Ein Test setzt die Grenzen durch.
- **Details benennen die Sache.** Eine E-Mail über einen Schlüssel, ein Budget oder ein Modell beantwortet was, wo und wann von selbst: Eine Liste aus Bezeichnungen und Werten unter dem Satz nennt die Organisation, das Projekt, den Anbieter, den Schlüssel und seine letzten vier Zeichen, den verbrauchten Betrag und wann er zurückgesetzt wird, die Antwort des Anbieters und die Zeitpunkte. Jede sendende Stelle übergibt, was sie weiß; die E-Mail rät nie.
- **Keine nackten Links.** Das HTML zeigt nie eine URL; der Button trägt sie. Der Textteil enthält den vollständigen Link auf einer eigenen Zeile und sagt dieselben Worte in derselben Reihenfolge.
- **Fußzeile.** „Gesendet von Kaeso unter“ und der Host der Installation, aus `KAESO_PUBLIC_URL`. Hinweise sagen außerdem, warum sie ankamen: Sicherheits- und Organisationshinweise werden immer gesendet, und Warnungen sagen, wie man sie abschaltet. Benachrichtigungen nennen das Recht, wegen dessen die Person sie erhält, verlinken ihre Einstellungen und, außer bei Sicherheits-E-Mails, eine Ein-Klick-Abmeldung, die auch `List-Unsubscribe` und `List-Unsubscribe-Post` tragen.

Text und HTML jeder E-Mail sind Snapshots in `crates/kaeso-mail/src/snapshots/`.

## Wie sie gebaut wird

| Datei | Aufgabe |
| --- | --- |
| `templates.rs` | Eine Funktion pro E-Mail, der Katalog dessen, [was Kaeso sendet](../../self-hosting/email.md#was-kaeso-sendet): Betreff, Überschrift, Satz, Button, leise Zeile und warum sie gesendet wird, in jeder Sprache |
| `notices.rs` | Die Benachrichtigungen: ein `Fact` pro Art von Ereignis, gesetzt als `[Organisation oder Projekt] Tatsache` mit Grund und Links, und die tägliche Zusammenfassung |
| `locale.rs` | `Locale` (`en`, `de`, `zh-CN`) sowie Datumsangaben und Zahlen, wie jede Sprache sie schreibt |
| `layout.rs` | Das gemeinsame Layout, als HTML mit maud und als reiner Text |
| `style.rs` | Die Werte von `@logicplanes/kaeso-tokens` in seinem Makro `token!`, der Inline-Stil jeder Rolle und das Stylesheet. Das Layout schreibt nie eine eigene Farbe oder Größe |
| `assets.rs`, `assets/` | Die Bilder der Wortmarke und der Titelbilder und ihre SVG-Quellen |
| `mailer.rs` | Eine Warteschlange für bis zu 1.024 E-Mails, drei Wiederholungen nach 1, 5 und 30 Sekunden, die Transporte SMTP, Log und Speicher sowie die MIME-Struktur |

- **E-Mail-taugliches HTML.** Tabellen geben die Struktur vor, und alles, was ein Inline-Stil sein kann, ist einer. Ein `<style>`-Block enthält nur Farbschemata, Resets und den Dunkelmodus. Bedingte Kommentare geben Outlook für Windows eine feste Spalte von 560 px. Keine Webfonts, externen Bilder, Skripte oder Tracking-Pixel.
- **Der Button** ist ein Link mit Innenabstand und vollem Radius. Für Outlook für Windows, das einen Link weder auffüllen noch runden kann, ist er ein VML-`roundrect`, dessen Breite aus der Beschriftung kommt, sodass er nie umbricht.
- **Escaping.** maud escapet die Namen, die Menschen wählen, etwa Organisationen, Schlüssel und Projekte, in Text und Attributen. Das einzige nicht escapete Markup ist fest: der Doctype, die bedingten Kommentare für Outlook und der VML-Button, der escapet, was er einbettet. Tests schicken Markup und Anführungszeichen durch jeden Namen.
- **MIME.** `multipart/alternative` mit dem Text zuerst und einem HTML-Teil, der die Bilder, die er zeigt, per `Content-ID` eingebettet trägt, die beiden PNGs der Wortmarke oder das Titelbild, sodass Mail-Apps sie nicht als Anhänge aufführen.

## Benachrichtigungen

`services::notifications` in `kaeso-server` entscheidet, wer eine Benachrichtigung erhält und wann.

1. Jedes Audit-Ereignis wird für die Anfrage oder den Job gesammelt, die es geschrieben haben. Der Dispatcher läuft, sobald die Anfrage erfolgreich geantwortet hat oder der Job fertig ist, sodass eine zurückgerollte Änderung niemanden benachrichtigt.
2. `dispatch::kinds` ordnet jeder Audit-`Action` die Arten zu, über die sie benachrichtigt, in einem `match` ohne Platzhalter, sodass eine neue Aktion eine Entscheidung braucht. Ereignisse ohne Aktion rufen `system` direkt auf: eine Verbindung oder ein Anbieterkonto, das sich abmeldet, ein zurückgehaltenes Tool, ein dreimal nacheinander abgelehnter Anbieter-Schlüssel, ein abgeschlossener Export, ein aufgebrauchtes hartes Budget und eine neue Version.
3. Empfänger ergeben sich aus Rechten, nie aus Rollennamen: aus den Rechten der Art im Benachrichtigungskatalog von `kaeso-core`, gehalten in der Organisation oder dem Projekt, aus den Instanz-Admins oder aus der Person, um die es geht. Wer gehandelt hat, bleibt außen vor, außer bei Sicherheits-E-Mails.
4. Die Einstellung jeder Person wählt sofort, täglich oder aus, und eine stummgeschaltete Organisation stoppt alles außer Sicherheits-E-Mails. `notifications_sent` hält eine E-Mail pro Thema und Tag, und über 20 sofortige E-Mails pro Stunde kommt der Rest in `notification_digest`.
5. Der Job `notifications` läuft jede Minute. Um 8:31 Uhr in der Zeitzone der Installation sendet er wartende Zusammenfassungen, am 1. den monatlichen Kostenbericht und montags die Wochenübersicht. Die Wochenübersicht nennt die Woche und jede Organisation, und ihr Button öffnet die Nutzung dieser Woche; eine Zeile der Zusammenfassung behält die Organisation und das Projekt, um die es ging, sodass die Zusammenfassung ihre Zeilen danach gruppiert.

## Sprachen

`services::mail_lang` in `kaeso-server` wählt die Sprache. Die Wahl der Person in ihren Kontoeinstellungen gewinnt (`users.locale`; `null` heißt Automatisch). Sonst nimmt es die erste Sprache, die Kaeso schreibt, aus dem `Accept-Language` der Anfrage, die die E-Mail ausgelöst hat.

Kann die Datenbank nicht antworten, entscheiden die Rückfallregeln, sodass die Abfrage nie eine E-Mail aufhält. [E-Mail](../../self-hosting/email.md#sprachen) nennt die Regeln für jede Art von Empfänger.

Die Funktion jeder E-Mail hat ein `match` auf die Sprache, mit ganzen Sätzen und namentlich eingesetzten Werten, nie zusammengesetzten Bruchstücken. Deutsch spricht Menschen mit „Sie“ an. Chinesisch nutzt „您“, Satzzeichen in voller Breite und ein Leerzeichen zwischen chinesischen und lateinischen Wörtern oder Ziffern, das `locale::zh` an den Nahtstellen setzt. Datumsangaben und Zahlen folgen der Sprache:

| | Englisch | Deutsch | Chinesisch |
| --- | --- | --- | --- |
| Datum | `14 Oct 2026` | `14. Okt. 2026` | `2026年10月14日` |
| Monat | `October 2026` | `Oktober 2026` | `2026年10月` |
| Betrag | `1,250` | `1.250` | `1,250` |
| Dollar | `$1,250.00` | `1.250,00 $` | `$1,250.00` |
| Prozent | `80%` | `80 %` | `80%` |

## Die Wortmarke

E-Mails tragen die Wortmarke als zwei eingebettete PNGs, hell und dunkel, zusammen etwa 6 KB. Die Adresse einer selbst betriebenen Installation kann intern sein, und viele Mail-Apps blockieren externe Bilder, verlinkte Bilder könnten also fehlen.

Jedes PNG hat die dreifache Anzeigegröße und wird aus `assets/wordmark-light.svg` und `assets/wordmark-dark.svg` gerendert. Ein Test rendert die SVGs erneut mit resvg und schlägt fehl, wenn die committeten PNGs abweichen. So schreiben Sie sie neu:

```bash
KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets
```

## Titelbilder

Ankündigungen beginnen mit einem Titelbild statt der Wortmarke: Einladungen, Links zur Registrierung und Einrichtung, die Test-E-Mail, eine neue App, eine angenommene Einladung, die Wochenübersicht, die Zusammenfassung, der monatliche Kostenbericht und eine neue oder installierte Version. Warnungen, Sicherheits- und Organisations-E-Mails nie; sie bleiben kurz und beginnen mit der Wortmarke.

Jedes Titelbild ist die Release-Grafik von Kaeso, gezeichnet von `drawReleaseArt` in `packages/ui` in ihrer breiten Form, mit der Wortmarke in der Ecke, die die Grafik frei lässt. Es reist eingebettet wie die Wortmarke: ein PNG mit 1120 × 480 Pixeln, angezeigt mit 560 × 240, unter 60 KB, mit Alternativtext in jeder Sprache. Es enthält keinen Text außer der Wortmarke, braucht also keine Übersetzung und sieht hell und dunkel gleich aus. `scripts/mail-covers.mjs` schreibt die SVGs, ein Test daneben prüft sie gegen die Release-Grafik, und der Asset-Test rendert die PNGs daraus. So zeichnen Sie sie neu, nachdem sich die Release-Grafik geändert hat:

```bash
node scripts/mail-covers.mjs
KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets
```

Die E-Mail zu einer neuen Version schreibt die Version als Text, nicht ins Titelbild. Sie führt die ersten vier Änderungen der Release-Notizen auf und wie viele weitere es gibt, sagt, ob sich das Release im Update-Fenster selbst installiert, über die Update-Seite oder von Hand, und verlinkt die Notizen.

## Dunkelmodus

Kaeso legt den Dunkelmodus selbst fest, statt ihn der Invertierung jeder Mail-App zu überlassen. Mail-Apps lassen sich in drei Arten einteilen, und jede bekommt eine eigene Ebene:

1. **Apps, die `prefers-color-scheme` folgen**, etwa Apple Mail und Thunderbird. Das Dokument erklärt beide Farbschemata, sodass sie es nicht invertieren, und `@media (prefers-color-scheme: dark)` schaltet jede Rolle mit `!important` auf ihr dunkles Token.
2. **Outlook.com und das neue Outlook**, die E-Mails selbst abdunkeln und das Umgefärbte mit `data-ogsc` und `data-ogsb` markieren. Dieselben Regeln unter diesen Attributen ersetzen ihre Schätzung durch die dunklen Werte von Kaeso.
3. **Apps, die ungefragt invertieren**, etwa die Gmail-Apps. Das helle Design übersteht die Invertierung. Sein Text ist fast schwarz, und sein Button wird zur hellen Pille des dunklen Themes. Ein weißer Rand umgibt die helle Wortmarke, und die dunkle Wortmarke bleibt verborgen.

Das Stylesheet kommt aus demselben Token-Makro wie die Inline-Stile. Tests prüfen, dass jede dunkle Regel `!important` ist und jede Rolle in jeder Ebene eine Regel hat.

## E-Mails ansehen

Der [Entwicklungs-Stack](../stack.md) enthält Mailpit, dessen Weboberfläche unter http://127.0.0.1:58025 eine HTML-Kompatibilitätsprüfung und eine dunkle Vorschau bietet. Senden Sie jede E-Mail in jeder Sprache dorthin:

```bash
cargo test -p kaeso-mail --test smtp
```

Der Test prüft, dass jede E-Mail mit beiden Teilen und genau den Bildern, die sie zeigt, eingebettet ankommt und dass die HTML-Prüfung von Mailpit genug Unterstützung über die Mail-Apps meldet. Danach löscht er seine Nachrichten. `KAESO_TEST_SMTP_URL` und `KAESO_TEST_MAIL_API` lenken ihn anderswohin.
