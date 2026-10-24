---
title: Anmeldung und Single Sign-on
nav: Anmeldung und SSO
description: Legen Sie Anmeldemethoden fest, betreiben Sie Kaeso ohne Passwörter und kommen Sie wieder hinein, wenn nichts geht.
order: 7
---

Personen melden sich an der Konsole mit einem Passwort an, mit einem Anmeldelink per E-Mail oder über den Identitätsanbieter ihrer Organisation per OpenID Connect. Zwei [Einstellungen zur Anmeldung](configuration.md#anmeldung) schalten Passwörter und Anmeldelinks für die ganze Installation ein oder aus. Single Sign-on richtet jede Organisation selbst ein, nicht die Installation: Es funktioniert für eine Organisation, sobald ihr Admin den Identitätsanbieter auf der Seite **Sicherheit** der Organisation verbindet ([Organisationen und Projekte](../console/organizations-and-projects.md)), und jede Organisation kann das.

## Die Anmeldeseite

Die Anmeldeseite fragt immer zuerst nach der E-Mail-Adresse. Dann bietet sie an, was für diese Adresse funktioniert: ein Passwort, einen Anmeldelink und Single Sign-on, wenn die Adresse zu einer bestätigten E-Mail-Domain einer Organisation mit Identitätsanbieter gehört ([E-Mail-Domains bestätigen](../console/organizations-and-projects.md#e-mail-domains-bestätigen)). Einer Adresse an einer anderen Domain oder an einer Domain, die ihre Organisation noch nicht bestätigt hat, bietet sie nie Single Sign-on an.

Auf dem Button steht **Weiter mit** und der Name, den der Admin dem Anbieter gegeben hat, ohne Namen **Weiter mit SSO**. Ist Single Sign-on der einzige Weg hinein für die Adresse und hat die Verbindung **Ohne Klick starten** eingeschaltet, geht die Seite gleich nach der Adresse zum Anbieter weiter, ohne den Button. Nach einer Abmeldung oder wenn der Anbieter eine Anmeldung abgelehnt hat, fragt die Seite erneut nach der Adresse und zeigt den Grund.

Single Sign-on gibt es erst nach der Adresse: Kein Link startet es für eine Organisation, sodass ein Link auf einer anderen Website einen Browser nicht über einen Identitätsanbieter in das Konto einer anderen Person bringen kann. Für ein Lesezeichen oder eine Intranetseite verlinken Sie die Anmeldeseite, etwa `https://kaeso.example.com/sign-in`; mit **Ohne Klick starten** gehen Mitglieder zum Anbieter weiter, sobald sie ihre Adresse eingegeben haben.

Single Sign-on ersetzt auch nie die Sitzung eines anderen Kontos. Kommt vom Anbieter ein anderes Konto zurück, während der Browser angemeldet ist, bleibt er angemeldet, und die Seite meldet `signed_in_as_other`; zum Wechseln melden Sie sich zuerst ab.

Nach der Anmeldung kehren Personen zu der Konsolenseite zurück, die sie zuerst geöffnet hatten, auch über den Identitätsanbieter.

## Kürzliche Anmeldungen

Verlangt eine Aktion eine kürzliche Anmeldung, bittet die Konsole den Anbieter, die Person erneut anzumelden (`prompt=login`, `max_age=0`), wie OpenID Connect es vorsieht. Nur eine Antwort, nach der sich die Person während der Bestätigung angemeldet hat, zählt; eine Bestätigung bedeutet also immer eine frische Anmeldung.

Ein Anbieter kann eine Person auch ohne Nachfrage mit einer eigenen, älteren Sitzung anmelden. Eine Sitzung aus Single Sign-on zählt deshalb ab dem `auth_time` des Anbieters, nicht ab der Rückkehr zu Kaeso, und gilt nur als kürzliche Anmeldung, solange dieser Zeitpunkt höchstens 15 Minuten zurückliegt. Kaeso misst ihn am Ausstellungszeitpunkt `iat` des Tokens, auf der Uhr des Anbieters; die Uhren beider Server müssen also nicht übereinstimmen. Ohne `auth_time` gilt die Sitzung von Anfang an als älter als 15 Minuten.

Manche Anbieter können keine kürzliche Anmeldung bestätigen: Sie lassen `auth_time` weg, oder sie ignorieren `max_age=0` und antworten mit einer eigenen, älteren Anmeldung. OpenID Connect verlangt eine frische Anmeldung und `auth_time`, sobald `max_age` gesendet wird, doch Kaeso erkennt einen Anbieter, der das ignoriert, erst an einer gescheiterten Bestätigung. Die Konsole bietet Single Sign-on zum Bestätigen deshalb nur an, wenn das Konto keinen anderen Weg hat: kein Passwort, keinen Anmeldelink und keinen Link, um ein Passwort festzulegen.

Eine Bestätigung ohne frische Anmeldung wird mit `cannot_confirm` abgelehnt, und Kaeso vermerkt an der Verbindung, dass der Anbieter nicht bestätigen kann. Die Konsole bietet ihn dann nicht mehr an, und die Seite **Sicherheit** der Organisation zeigt **Kürzliche Anmeldung**. Für ein Konto, dessen Organisation Single Sign-on verlangt ([verwaltete Konten](#verwaltete-konten)), bestätigt sonst nichts; es kann Aktionen, die eine kürzliche Anmeldung verlangen, also nicht ausführen. Kaeso vergisst den Vermerk, wenn sich Issuer oder Client-ID ändern; andere Anmeldungen vermerken nichts, da ein Anbieter ohne `max_age` seine Anmeldung wiederverwenden darf.

## Wer sich über Single Sign-on anmeldet

- Nur Adressen unter den bestätigten E-Mail-Domains der Verbindung ([E-Mail-Domains bestätigen](../console/organizations-and-projects.md#e-mail-domains-bestätigen)). Jede Domain bestätigt auf einer Installation eine Organisation.
- Nur Adressen, die der Anbieter mit `email_verified` als bestätigt kennzeichnet. Für einen Anbieter, der den Claim nie sendet, kann eine Verbindung **Bestätigte Adressen verlangen** abschalten; eine als unbestätigt gekennzeichnete Adresse bleibt abgewiesen.
- Wer neu bei Kaeso ist, tritt der Organisation mit einer Einladung an seine Adresse bei oder mit der Rolle, die die Verbindung Personen ohne Einladung gibt. Ohne beides erhält er keinen Zugang. Kaeso legt das Konto auch an, wo die [Einstellung zur Kontoerstellung](../console/instance-administration.md#installationseinstellungen-ändern) keine Registrierungen erlaubt, denn der Anbieter der Organisation bürgt für die Person.
- Kaeso verknüpft jedes Konto mit seiner Identität beim Anbieter, dem Aussteller und dem Subjekt des Anbieters. Spätere Anmeldungen folgen dieser Verknüpfung, nicht der E-Mail-Adresse.
- Ein bestehendes Konto meldet sich über Single Sign-on erst an, wenn seine Inhaberin oder sein Inhaber es nach einer Anmeldung auf anderem Weg unter **Konto** › **Sicherheit** verknüpft ([Single Sign-on verknüpfen](../console/account.md#single-sign-on-verknüpfen)), und nur dort, wo es Mitglied ist. Um eine Einladung anzunehmen, meldet es sich zuerst auf anderem Weg an.
- Eine Adresse unter einer anderen Domain, etwa der eines öffentlichen Mail-Anbieters, kann sich nicht über den Anbieter der Organisation anmelden, auch nicht mit Einladung.

Was eine Anmeldung über einen Anbieter öffnet, hängt davon ab, ob seine Organisation das Konto verwaltet ([verwaltete Konten](#verwaltete-konten)).

Eine Verbindung kann keine Domain eines öffentlichen Mail-Anbieters wie `gmail.com` enthalten: Jede Adresse dort würde sich über den Anbieter einer einzigen Organisation anmelden.

## Verwaltete Konten

Eine Organisation mit Single Sign-on verwaltet die Konten an ihren bestätigten E-Mail-Domains. Kaeso ermittelt das bei jeder Anfrage aus der Adresse des Kontos; eine neue Adresse, eine Domain, die ihre Bestätigung verliert, oder eine entfernte Verbindung ändert es also sofort. Jede Domain bestätigt eine Organisation, sodass höchstens eine Organisation ein Konto verwaltet. Die übrigen Mitglieder der Organisation sind Gäste: Personen, die ihr angehören, deren Adresse aber an einer anderen Domain liegt.

Die Anmeldung eines verwalteten Kontos über den Anbieter seiner Organisation ist eine vollständige Anmeldung, wie bei einem Arbeitskonto, das zu einem Unternehmen gehört. Sie öffnet jede Organisation, der die Person angehört, jede mit ihren eigenen Anforderungen, und Änderungen am Konto. Eine Anmeldung über den Anbieter einer Organisation, die das Konto nicht verwaltet, öffnet nur diese Organisation. Das geschieht, wenn die Adresse an einer anderen Domain liegt, etwa nachdem sie sich geändert hat, oder wenn die Domain nicht mehr bestätigt ist.

| | Verwaltetes Konto, über den Anbieter seiner Organisation | Über den Anbieter einer anderen Organisation | Passwort oder Anmeldelink |
| --- | --- | --- | --- |
| Organisationen | Jede, der die Person angehört | Nur die Organisation des Anbieters | Jede, außer einer, die das Konto verwaltet und Single Sign-on verlangt |
| Zwei-Faktor-Pflicht einer Organisation | Durch den Anbieter erfüllt | In der Organisation des Anbieters erfüllt | Braucht einen Code |
| Änderungen am Konto | Erlaubt, mit kürzlicher Anmeldung | Brauchen ein Passwort oder einen Anmeldelink | Erlaubt, mit kürzlicher Anmeldung |
| Verwaltung der Instanz | Nur über die Organisation, die für Instanz-Admins gewählt ist | Braucht ein Passwort oder einen Anmeldelink | Für Instanz-Admins erlaubt |

Die Adresse eines verwalteten Kontos gehört dem Anbieter. `POST me/email` lehnt eine Änderung mit `Your organization manages this address; change it at its identity provider.` ab, und ein Link zur Bestätigung einer früheren Änderung funktioniert nicht mehr. Bringt eine Anmeldung über den Anbieter eine andere Adresse an einer der bestätigten Domains der Organisation, ändert Kaeso die Adresse des Kontos darauf, denn Issuer und Subjekt verknüpfen das Konto, nicht die Adresse. Das Audit-Log der Organisation hält das als `user.email_change` fest. Eine Adresse, die ein anderes Konto hat, wird nie übernommen, und der Anbieter einer Organisation, die das Konto nicht verwaltet, ändert sie nie.

**Konto** › **Profil** zeigt **Verwaltet von** und den Namen der Organisation, und die Seite **Mitglieder** einer Organisation kennzeichnet Gäste ([nach einer Anmeldung über Single Sign-on](../console/account.md#nach-einer-anmeldung-über-single-sign-on)).

### Single Sign-on verlangen

Ist **Single Sign-on verlangen** eingeschaltet, melden sich die Konten, die eine Organisation verwaltet, überall auf der Installation nur über ihren Anbieter an. Die Anmeldeseite bietet ihnen allein den Anbieter an, und ein Passwort, ein Anmeldelink, eine Registrierung oder eine Einladung ohne Konto wird mit `Your organization requires single sign-on; continue with its identity provider.` abgelehnt. Das Einschalten der Pflicht beendet ihre Sitzungen aus einem Passwort oder einem Anmeldelink, außer der Sitzung, die sie einschaltet; ebenso das Bestätigen einer weiteren Domain, während die Pflicht gilt.

Gäste behalten ihre eigene Anmeldung und öffnen die Organisation damit, unter ihrer Zwei-Faktor-Pflicht. Ein Anmeldelink vom Server meldet ein verwaltetes Konto weiterhin an ([wieder hineinkommen](#wieder-hineinkommen)).

### Instanz-Admins

Standardmäßig verwaltet der Identitätsanbieter keiner Organisation die Instanz; wer den Anbieter einer Organisation verwaltet, kann sich also nicht selbst zum Admin der Instanz machen. Um es einer Organisation zu erlauben, wählen Sie sie unter **Instanz** › **Allgemein** › **Single Sign-on für Instanz-Admins** ([Installationseinstellungen ändern](../console/instance-administration.md#installationseinstellungen-ändern)). Die Anmeldung eines Instanz-Admins über den Anbieter dieser Organisation verwaltet dann die Instanz, solange die Organisation sein Konto verwaltet.

Die Wahl verlangt die kürzliche Anmeldung eines Instanz-Admins mit Passwort oder Anmeldelink, nie eine über Single Sign-on, und das Audit-Log der Instanz hält sie als `instance.admin_sso_update` fest. Wählen lässt sich nur eine Organisation mit Verbindung. Verliert sie ihre Verbindung später, bleibt die Einstellung wirkungslos, und **Allgemein** weist darauf hin.

Kann sich kein Instanz-Admin so anmelden, wählt ein Operator die Organisation stattdessen auf dem Server über ihre ID, nach denselben Regeln und mit demselben Audit-Eintrag; `none` wählt keine:

```bash
docker compose exec gateway kaeso instance admin-sso org_…
```

## Ohne Passwörter und Anmeldelinks betreiben

Damit sich Personen nur über die Identitätsanbieter von Organisationen anmelden, schalten Sie vor dem ersten Start Passwörter und Anmeldelinks ab:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_AUTH_PASSWORD: "false"
      KAESO_AUTH_MAGIC_LINK: "false"
```

1. Geben Sie dem ersten Admin eine Adresse unter einer Domain, die der Identitätsanbieter abdecken wird, damit er sich später darüber anmelden kann.
2. Öffnen Sie den Einrichtungslink. Die Einrichtungsseite fragt nach keinem Passwort und öffnet danach die Seite **Sicherheit** der Organisation.
3. Verbinden Sie den Identitätsanbieter in derselben Sitzung.
4. Wählen Sie unter **Konto** › **Sicherheit** die Option **Verknüpfen** neben der Organisation und melden Sie sich beim Anbieter an. Das verknüpft Ihr Konto und prüft, dass die Verbindung funktioniert.

Die Anmeldeseite fragt weiterhin zuerst nach der E-Mail-Adresse und bietet den Anbieter ihrer Organisation an. Eine Adresse an keiner bestätigten Domain einer Organisation kommt nicht hinein, und die Seite sagt das.

Eine Anmeldung über Single Sign-on verwaltet die Instanz erst, wenn ein Instanz-Admin die Organisation dafür wählt ([Instanz-Admins](#instanz-admins)). Bis dahin melden Sie sich für die Verwaltung der Instanz mit einem [einmaligen Anmeldelink](#wieder-hineinkommen) vom Server an.

> [!IMPORTANT]
> Lassen Sie **Single Sign-on verlangen** ausgeschaltet, bis die Verbindung funktioniert. Auf einer Installation ohne Passwörter bringt es nichts, und es sperrt die Organisation für jede Sitzung ihrer verwalteten Konten, die nicht von ihrem Anbieter stammt, auch für eine über den Anmeldelink eines Betreibers.

## Wieder hineinkommen

Wenn kein Weg hinein funktioniert, geben Sie auf dem Server einen einmaligen Anmeldelink für ein bestehendes Konto aus. Das hilft, wenn der Identitätsanbieter ausgefallen oder falsch eingerichtet ist oder die Einrichtungssitzung endete, bevor Single Sign-on funktionierte:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

Der Link funktioniert einmal, innerhalb von 15 Minuten, auch wenn Anmeldelinks per E-Mail abgeschaltet sind; Passwörter müssen Sie also nie wieder einschalten. Wer den Link hat, kann sich als diese Person anmelden. Kaeso gibt ihn nur in Ihrem Terminal aus, protokolliert oder sendet ihn nie, und das Audit-Log hält ihn als `user.sign_in_link` fest. Ein deaktiviertes Konto erhält erst einen Link, wenn Sie es wieder aktivieren. Bei eingeschalteter Zwei-Faktor-Authentifizierung fragt auch der Link nach einem Code; hat die Person ihren Authenticator verloren, entfernt `kaeso user reset-2fa` sie zuerst.

Der Link meldet ein verwaltetes Konto auch an, während seine Organisation Single Sign-on verlangt. Diese Sitzung öffnet das Konto der Person, ihre anderen Organisationen und, bei einem Instanz-Admin, die Verwaltung der Instanz; die Organisation, die Single Sign-on verlangt, bleibt für sie gesperrt.

## Nächste Schritte

- [Instanzverwaltung](../console/instance-administration.md): Verwalten Sie Benutzer, Organisationen und die Einstellungen der Instanz.
- [Fehlerbehebung](troubleshooting.md): Beheben Sie häufige Probleme einer Installation.
