---
title: E-Mail
description: Verbinden Sie einen SMTP-Server, testen Sie ihn und sehen Sie, welche E-Mails Kaeso in welcher Sprache sendet.
order: 6
---

Das Gateway sendet jede E-Mail selbst, über SMTP: Anmeldelinks, Einladungen, Passwort-Resets, Warnungen zum Ablauf von Schlüsseln, Budgetwarnungen und Sicherheitshinweise. Ohne SMTP-Server startet es nicht, außer Sie lassen E-Mails nur protokollieren.

## SMTP einrichten

Tragen Sie Ihren SMTP-Server in die `.env` ein:

```bash title=".env"
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_PORT=587
KAESO_SMTP_TLS=starttls
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
```

Übernehmen Sie die Werte mit `docker compose up -d --wait`. Ohne `KAESO_SMTP_FROM` sendet Kaeso von `kaeso@` unter Ihrer Domain; setzen Sie also eine Adresse, von der Ihr SMTP-Server senden darf. Für einen Server ohne Anmeldung lassen Sie Benutzer und Passwort weg. Die [Konfiguration](configuration.md#e-mail) listet jede E-Mail-Einstellung und ihre Werte.

## Test-E-Mail senden

1. Öffnen Sie in der Konsole **Einstellungen** › **Instanz** › **Einstellungen**. Der Abschnitt **E-Mail** zeigt den Transport, den SMTP-Host und den Absender.
2. Wählen Sie **Test-E-Mail senden**. Sie geht an Sie oder an die Adresse, die Sie eingeben.
3. Scheitert sie oder kommt sie nicht an, lesen Sie die Antwort des SMTP-Servers im Protokoll des Gateways mit `docker compose logs gateway`.

Die [Fehlerbehebung](troubleshooting.md#e-mails-kommen-nicht-an) nennt die üblichen Ursachen.

## Kaeso ohne Mailserver ausprobieren

Setzen Sie `KAESO_MAIL_TRANSPORT=log` in der `.env`. Das Gateway protokolliert dann die Art jeder E-Mail, statt sie zu senden, nie aber Empfänger oder Inhalt.

Die Anmeldung mit Passwort funktioniert weiter, und `kaeso setup-link` gibt den Einrichtungslink in jedem Fall aus. Anmeldelinks, Einladungen und Warnungen kommen nie an.

## Was Kaeso sendet

| E-Mail | Gesendet, wenn |
| --- | --- |
| Einladung | Jemand eine Person in eine Organisation einlädt |
| Einladung zur Instanz | Ein Instanz-Admin eine Person in die Installation einlädt |
| Anmeldelink | Jemand einen Anmeldelink anfordert |
| Passwort-Reset | Jemand sein Passwort zurücksetzen will |
| Adressänderung | Jemand seine E-Mail-Adresse ändert; geht an die neue Adresse |
| Registrierung | Sich jemand registriert, wo die Instanz es erlaubt |
| Passwort geändert | Ein Passwort sich ändert |
| Zwei-Faktor an | Jemand die Zwei-Faktor-Authentifizierung einschaltet |
| Zwei-Faktor aus | Jemand die Zwei-Faktor-Authentifizierung abschaltet |
| Zwei-Faktor zurückgesetzt | Ein Instanz-Admin oder `kaeso user reset-2fa` sie zurücksetzt |
| Adresse geändert | Eine E-Mail-Adresse sich ändert |
| Abgemeldet | Jede Sitzung einer Person endet |
| Konto deaktiviert | Ein Instanz-Admin ein Konto deaktiviert |
| Löschung einer Organisation | Eine Organisation zur Löschung vorgemerkt wird; geht an ihre Mitglieder und an Instanz-Admins |
| Inhaberschaft | Jemand eine Person zum Inhaber macht |
| Ablauf eines Schlüssels | Einer der API-Schlüssel einer Person bald abläuft |
| Budgetwarnung | Ein Budget eine seiner Warnschwellen erreicht |
| Test | Ein Instanz-Admin eine Test-E-Mail sendet |

Sicherheits- und Organisationshinweise werden immer gesendet. Welche Warnungen sie erhält, wählt jede Person unter **Einstellungen** › **Konto** › **Benachrichtigungen**, und jede Warnung sagt in ihrer Fußzeile, wie sie sich abschalten lässt.

Jede E-Mail hat einen HTML- und einen Textteil, hell und dunkel. Ihre Bilder sind in die E-Mail eingebettet, sodass sie nichts von Ihrer Installation lädt und keine Tracking-Pixel enthält.

## Sprachen

Jede E-Mail gibt es auf Englisch, Deutsch und vereinfachtem Chinesisch. Kaeso wählt die Sprache für jeden Empfänger:

| Empfänger | Sprache |
| --- | --- |
| Eine Person mit Konto | Ihre Wahl unter **Einstellungen** › **Konto** › **Darstellung**. Bei Automatisch die Sprache des Browsers, der die E-Mail ausgelöst hat, sonst Englisch |
| Jemand ohne Konto, für eine Einladung oder einen Registrierungslink | Die Sprache der Person, die eingeladen oder angefragt hat, sonst Englisch |
| Empfänger von Ablaufwarnungen, Budgetwarnungen und Löschhinweisen | Die Wahl jedes Empfängers, sonst Englisch |

## Zustellung

E-Mails warten in einer Warteschlange im Speicher des Gateways, bis zu 1.024 Stück, und bis zu 16 gehen gleichzeitig hinaus:

- Scheitert der SMTP-Server vorübergehend, versucht es das Gateway noch dreimal, nach 1, 5 und 30 Sekunden.
- Lehnt der Server eine E-Mail endgültig ab, versucht es das Gateway nicht erneut.
- Ist die Warteschlange voll, verwirft das Gateway die E-Mail.
- Wenn das Gateway anhält, wartet es bis zu 10 Sekunden auf E-Mails in der Warteschlange; was dann noch nicht gesendet ist, geht verloren.

Das Gateway protokolliert jede E-Mail, die es nicht zustellen kann. `kaeso_emails_total` zählt E-Mails nach `outcome`, `sent`, `failed` oder `dropped` ([Zustand und Metriken](monitoring.md)).

## Nächste Schritte

- [Anmeldung und Single Sign-on](sign-in.md): Legen Sie fest, wie sich Personen anmelden.
- [Fehlerbehebung](troubleshooting.md#e-mails-kommen-nicht-an): Finden Sie heraus, warum eine E-Mail nicht ankommt.
