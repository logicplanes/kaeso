---
title: E-Mail
description: Wie Kaeso E-Mails sendet, was es sendet, in welcher Sprache und wie sich das testen lässt.
order: 5
---

Kaeso sendet E-Mails für Anmeldelinks, Einladungen, das Zurücksetzen von Passwörtern, Warnungen vor ablaufenden Schlüsseln, Budget-Warnungen und Sicherheitshinweise. Das Gateway sendet jede E-Mail selbst, über SMTP.

## SMTP einrichten

Den SMTP-Server in `.env` eintragen und mit `docker compose up -d` übernehmen:

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
| `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD` | Beide weglassen für einen Server, der keine Anmeldung braucht; ein Passwort braucht einen Benutzer |
| `KAESO_SMTP_FROM` | Der Absender jeder E-Mail, als Adresse oder `Name <address>`. Die Compose-Datei setzt `Kaeso <kaeso@` und die eigene Domain; eine Adresse verwenden, von der der SMTP-Server senden darf |

Jede Einstellung steht unter [Konfiguration](configuration.md#e-mail). Ohne `KAESO_SMTP_HOST` startet das Gateway nicht, außer E-Mails werden nur protokolliert.

## Ohne Mailserver

Um Kaeso ohne Mailserver auszuprobieren, `KAESO_MAIL_TRANSPORT=log` setzen. Das Gateway protokolliert dann nur, dass eine E-Mail gesendet worden wäre, mit ihrer Art, nie mit Empfänger oder Inhalt. Die Anmeldung mit Passwort funktioniert, und der Einrichtungslink steht in jedem Fall im Log des Gateways; Anmeldelinks, Einladungen und Warnungen werden nicht zugestellt.

## Was Kaeso sendet

| E-Mail | Betreff | Gesendet, wenn |
| --- | --- | --- |
| Einladung | {inviter} hat Sie zu {org} auf Kaeso eingeladen | Jemand eine Person in eine Organisation einlädt |
| Einladung in die Instanz | {inviter} hat Sie zu Kaeso eingeladen | Ein Instanz-Admin eine Person in die Installation einlädt |
| Anmeldelink | Bei Kaeso anmelden | Jemand einen Anmeldelink anfordert |
| Passwort zurücksetzen | Kaeso-Passwort zurücksetzen | Jemand sein Passwort zurücksetzen will |
| Änderung der E-Mail-Adresse | Neue E-Mail-Adresse für Kaeso bestätigen | Jemand seine E-Mail-Adresse ändert; an die neue Adresse gesendet |
| Registrierung | Kaeso-Konto fertig erstellen | Sich jemand registriert, wo die Instanz es erlaubt |
| Passwort geändert | Ihr Kaeso-Passwort wurde geändert | Sich ein Passwort ändert |
| Zwei-Faktor an | Zwei-Faktor-Authentifizierung ist aktiviert | Jemand die Zwei-Faktor-Authentifizierung aktiviert |
| Zwei-Faktor aus | Zwei-Faktor-Authentifizierung ist deaktiviert | Jemand die Zwei-Faktor-Authentifizierung deaktiviert |
| Zwei-Faktor zurückgesetzt | Ihre Zwei-Faktor-Authentifizierung wurde zurückgesetzt | Ein Instanz-Admin oder `kaeso user reset-2fa` auf dem Server sie zurücksetzt |
| E-Mail-Adresse geändert | Ihre E-Mail-Adresse für Kaeso wurde geändert | Sich eine E-Mail-Adresse ändert |
| Abgemeldet | Sie wurden von Kaeso abgemeldet | Jede Sitzung einer Person beendet wurde |
| Konto deaktiviert | Ihr Kaeso-Konto wurde deaktiviert | Ein Instanz-Admin ein Konto deaktiviert |
| Löschung der Organisation | {org} ist zur Löschung vorgemerkt | Eine Organisation zur Löschung vorgemerkt wird; an ihre Mitglieder und an Instanz-Admins gesendet |
| Inhaberschaft | Sie sind jetzt Inhaber von {org} | Jemand eine Person zum Inhaber macht |
| Ablauf eines Schlüssels | Der API-Schlüssel {key} läuft am {date} ab | Einer der API-Schlüssel einer Person bald abläuft |
| Budget-Warnung | {scope} hat {percent} % des {budget} verbraucht | Ein Budget eine seiner Warnschwellen erreicht |
| Test | Kaeso-Test-E-Mail | Ein Instanz-Admin eine Test-E-Mail sendet |

Sicherheits- und Organisationshinweise werden immer gesendet. Warnungen nennen in ihrer Fußzeile, wie sie sich abschalten lassen; welche Warnungen sie erhalten, wählen Personen unter **Einstellungen** › **Konto** › **Benachrichtigungen**.

## Sprachen

Jede E-Mail gibt es auf Englisch, Deutsch und vereinfachtem Chinesisch, mit Datumsangaben und Zahlen so geschrieben, wie jede Sprache sie schreibt.

| Empfänger | Sprache |
| --- | --- |
| Eine Person mit Konto | Die Sprache, die sie unter **Einstellungen** › **Konto** gewählt hat; bei „Automatisch“ die Sprache des Browsers, der die E-Mail ausgelöst hat; sonst Englisch |
| Jemand ohne Konto, für eine Einladung oder einen Registrierungslink | Die Sprache der Person, die eingeladen oder angefordert hat, sonst Englisch; wer eingeladen wird und schon ein Konto hat, erhält die eigene Wahl |
| E-Mails, die ohne Browser gesendet werden: Warnungen vor ablaufenden Schlüsseln, Budget-Warnungen, Hinweise zur Löschung von Organisationen | Die Wahl jedes Empfängers, sonst Englisch |

## Wie E-Mails aussehen

Jede E-Mail hat das Aussehen der Konsole: die Kaeso-Wortmarke oben links, eine Überschrift, ein Satz, höchstens ein Button und eine Fußzeile, die die Installation nennt. Das HTML zeigt nie einen rohen Link, und ein Klartextteil sagt dieselben Worte, mit dem vollständigen Link.

- **Hell und dunkel.** Mail-Apps, die dem Dunkelmodus des Systems folgen, erhalten Kaesos dunkle Farben; der eigene Dunkelmodus von Outlook.com erhält dieselben Werte; Apps, die Farben von sich aus invertieren, erhalten ein helles Design, das dafür gebaut ist, die Invertierung zu überstehen.
- **Nichts von außen.** Die Bilder der Wortmarke reisen in der E-Mail mit, sodass sie immer erscheinen und nie etwas von der Installation laden. Es gibt keine Webfonts, externen Bilder, Skripte oder Tracking-Pixel.

## Zustellung

E-Mails warten in einer Warteschlange im Speicher des Gateways und werden nacheinander gesendet. Eine fehlgeschlagene Zustellung wird drei weitere Male versucht, nach 1, 5 und 30 Sekunden, dann aufgegeben und protokolliert. E-Mails, die noch in der Warteschlange stehen, wenn das Gateway anhält, werden nicht gesendet.

Die Metrik `kaeso_emails_total{kind, outcome}` zählt E-Mails nach `outcome`: `sent`, `failed` oder `dropped`, wenn die Warteschlange voll ist. Siehe [Zustand und Metriken](monitoring.md).

## Testen

In der Konsole **Einstellungen** › **Instanz** › **Einstellungen** öffnen. Der Abschnitt **E-Mail** zeigt den Transport, den SMTP-Host und den Absender; **Test-E-Mail senden** wählen, um eine an sich selbst oder an den eingegebenen Empfänger zu senden. Schlägt sie fehl oder kommt sie nicht an, steht im Log des Gateways, was der SMTP-Server geantwortet hat; mehr unter [Fehlerbehebung](troubleshooting.md#e-mails-kommen-nicht-an).
