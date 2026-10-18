---
title: E-Mail
description: Wie Kaeso E-Mails versendet, was es sendet, in welcher Sprache und wie es sich testen lässt.
order: 6
---

Kaeso sendet E-Mails für Anmeldelinks, Einladungen, das Zurücksetzen von Passwörtern, Warnungen vor ablaufenden Schlüsseln, Budget-Warnungen und Sicherheitshinweise. Das Gateway versendet jede E-Mail selbst über SMTP.

## SMTP einrichten

Den SMTP-Server in die `.env` eintragen und mit `docker compose up -d --wait` übernehmen:

```bash
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_PORT=587
KAESO_SMTP_TLS=starttls
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM=Kaeso <kaeso@example.com>
```

| Einstellung | Gut zu wissen |
| --- | --- |
| `KAESO_SMTP_TLS` | `starttls` auf Port 587, `tls` für implizites TLS (meist Port 465), `none` nur für einen lokalen Mail-Catcher |
| `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD` | Beide weglassen, wenn der Server keine Anmeldung verlangt; ein Passwort braucht einen Benutzer |
| `KAESO_SMTP_FROM` | Der Absender jeder E-Mail, als Adresse oder `Name <address>`. Standard ist `Kaeso <kaeso@` und die eigene Domain; eine Adresse verwenden, von der der SMTP-Server senden darf |

Ohne `KAESO_SMTP_HOST` startet das Gateway nicht, außer E-Mails werden nur protokolliert. [Konfiguration](configuration.md#e-mail) listet jede E-Mail-Einstellung.

## Ohne Mailserver

Um Kaeso ohne Mailserver auszuprobieren, `KAESO_MAIL_TRANSPORT=log` setzen. Das Gateway protokolliert dann nur, dass eine E-Mail gesendet worden wäre, mit ihrer Art, nie mit Empfänger oder Inhalt. Die Anmeldung mit Passwort funktioniert, und `kaeso setup-link` gibt den Einrichtungslink in jedem Fall aus, aber Anmeldelinks, Einladungen und Warnungen kommen nie an.

## Was Kaeso sendet

| E-Mail | Betreff | Gesendet, wenn |
| --- | --- | --- |
| Einladung | {inviter} hat Sie zu {org} auf Kaeso eingeladen | Jemand eine Person in eine Organisation einlädt |
| Einladung in die Instanz | {inviter} hat Sie zu Kaeso eingeladen | Ein Instanz-Admin eine Person in die Installation einlädt |
| Anmeldelink | Bei Kaeso anmelden | Jemand einen Anmeldelink anfordert |
| Passwort zurücksetzen | Kaeso-Passwort zurücksetzen | Jemand sein Passwort zurücksetzen will |
| Änderung der E-Mail-Adresse | Neue E-Mail-Adresse für Kaeso bestätigen | Jemand seine E-Mail-Adresse ändert; an die neue Adresse gesendet |
| Registrierung | Kaeso-Konto fertigstellen | Sich jemand registriert, wo die Instanz es erlaubt |
| Passwort geändert | Ihr Kaeso-Passwort wurde geändert | Sich ein Passwort ändert |
| Zwei-Faktor an | Zwei-Faktor-Authentifizierung ist eingeschaltet | Jemand die Zwei-Faktor-Authentifizierung aktiviert |
| Zwei-Faktor aus | Zwei-Faktor-Authentifizierung ist ausgeschaltet | Jemand die Zwei-Faktor-Authentifizierung deaktiviert |
| Zwei-Faktor zurückgesetzt | Ihre Zwei-Faktor-Authentifizierung wurde zurückgesetzt | Ein Instanz-Admin oder `kaeso user reset-2fa` auf dem Server sie zurücksetzt |
| E-Mail-Adresse geändert | Ihre E-Mail-Adresse für Kaeso wurde geändert | Sich eine E-Mail-Adresse ändert |
| Abgemeldet | Sie wurden von Kaeso abgemeldet | Jede Sitzung einer Person beendet wurde |
| Konto deaktiviert | Ihr Kaeso-Konto wurde deaktiviert | Ein Instanz-Admin ein Konto deaktiviert |
| Löschung der Organisation | {org} ist zur Löschung vorgemerkt | Eine Organisation zur Löschung vorgemerkt wird; an ihre Mitglieder und an Instanz-Admins gesendet |
| Inhaberschaft | Sie sind jetzt Inhaber von {org} | Jemand eine Person zum Inhaber macht |
| Ablauf eines Schlüssels | Der API-Schlüssel {key} läuft am {date} ab | Einer der API-Schlüssel einer Person bald abläuft |
| Budget-Warnung | {scope} hat {percent} % des {budget} verbraucht | Ein Budget eine seiner Warnschwellen erreicht |
| Test | Kaeso-Test-E-Mail | Ein Instanz-Admin eine Test-E-Mail sendet |

Sicherheits- und Organisationshinweise werden immer gesendet. Warnungen nennen in ihrer Fußzeile, wie sie sich abschalten lassen, und jede Person wählt unter **Einstellungen** › **Konto** › **Benachrichtigungen**, welche sie erhält.

## Sprachen

Jede E-Mail gibt es auf Englisch, Deutsch und vereinfachtem Chinesisch, mit Datumsangaben und Zahlen so, wie jede Sprache sie schreibt.

| Empfänger | Sprache |
| --- | --- |
| Eine Person mit Konto | Ihre Wahl unter **Einstellungen** › **Konto** › **Darstellung**; bei „Automatisch“ die Sprache des Browsers, der die E-Mail ausgelöst hat; sonst Englisch |
| Jemand ohne Konto, für eine Einladung oder einen Registrierungslink | Die Sprache der Person, die eingeladen oder angefordert hat, sonst Englisch; wer eingeladen wird und schon ein Konto hat, erhält die eigene Wahl |
| E-Mails, die ohne Browser gesendet werden: Warnungen vor ablaufenden Schlüsseln, Budget-Warnungen, Hinweise zur Löschung von Organisationen | Die Wahl jedes Empfängers, sonst Englisch |

## Wie E-Mails aussehen

Jede E-Mail sieht aus wie die Konsole: die Kaeso-Wortmarke oben links, eine Überschrift, ein Satz, höchstens ein Button und eine Fußzeile, die die Installation nennt. Der HTML-Teil zeigt nie einen rohen Link; der Textteil sagt dasselbe und enthält den Link vollständig.

- **Hell und dunkel.** Mail-Apps, die dem Dunkelmodus des Systems folgen, und der eigene Dunkelmodus von Outlook.com erhalten Kaesos dunkle Farben. Apps, die Farben selbst umkehren, erhalten ein helles Design, das die Umkehrung übersteht.
- **Nichts von außen.** Die Bilder der Wortmarke reisen in der E-Mail mit, werden also immer angezeigt und laden nie etwas von der Installation. Es gibt keine Webfonts, externen Bilder, Skripte oder Tracking-Pixel.

## Zustellung

E-Mails warten in einer Warteschlange im Speicher des Gateways, bis zu 1.024 Stück, und bis zu 16 werden gleichzeitig zugestellt. Scheitert der SMTP-Server vorübergehend, versucht das Gateway es dreimal erneut, nach 1, 5 und 30 Sekunden; lehnt er eine E-Mail endgültig ab, gibt es keinen neuen Versuch. In beiden Fällen wird eine nicht zustellbare E-Mail protokolliert. Beim Beenden wartet das Gateway bis zu 10 Sekunden auf die Warteschlange; was dann noch nicht gesendet ist, geht verloren.

`kaeso_emails_total{kind, outcome}` zählt E-Mails nach `outcome`: `sent`, `failed` oder `dropped`, wenn die Warteschlange voll war. Siehe [Zustand und Metriken](monitoring.md).

## Testen

In der Konsole **Einstellungen** › **Instanz** › **Einstellungen** öffnen. Der Abschnitt **E-Mail** zeigt Transport, SMTP-Host und Absender; **Test-E-Mail senden** schickt eine an die eigene Adresse oder an den eingegebenen Empfänger. Schlägt sie fehl oder kommt sie nicht an, steht die Antwort des SMTP-Servers im Log des Gateways, mehr unter [Fehlerbehebung](troubleshooting.md#e-mails-kommen-nicht-an).
