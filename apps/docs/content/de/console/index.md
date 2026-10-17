---
title: Die Konsole verwenden
description: Sich in der Konsole zurechtfinden, der Weboberfläche einer Kaeso-Installation.
order: 3
---

Die Konsole läuft unter der Domain der Installation, etwa `https://kaeso.example.com`. Angemeldet wird mit der E-Mail-Adresse und, je nachdem, was Installation und Organisation erlauben, mit Passwort, einem per E-Mail gesendeten Anmeldelink oder Single Sign-on; was die Anmeldeseite anbietet, erklärt [Anmeldung und Single Sign-on](../self-hosting/sign-in.md).

## Die Seitenleiste

Die Seitenleiste richtet sich danach, wo gerade gearbeitet wird:

- **In einer Organisation:** **Übersicht**, **Projekte** und **Anbieter**, darunter die Projekte der Organisation.
- **In einem Projekt:** **Übersicht**, **Playground**, **API-Schlüssel**, **Anfrageprotokolle**, **Modelle**, **Limits**, **Mitglieder**, **Dienstkonten**, **Audit-Log** und **Einstellungen**, unter einem Link zurück zur Organisation.

Das Menü am Fuß der Seitenleiste wechselt die Organisation und öffnet die **Organisationseinstellungen**, das eigene **Konto** und, für Instanz-Admins, die **Instanz**. Dort stehen auch **Organisation erstellen** und **Abmelden**.

## Rechte

Was die Konsole zeigt, hängt von den eigenen Rechten ab. Seiten ohne das nötige Recht fehlen in der Seitenleiste. Eine Aktion ohne das nötige Recht ist deaktiviert; ihr Tooltip nennt das Recht und wer es vergeben kann. Rollen und Rechte erklärt [Mitglieder und Rollen](members-and-roles.md).

Manche Aktionen verlangen eine Anmeldung innerhalb der letzten 15 Minuten; die Konsole fragt dann vor der Ausführung nach einer erneuten Anmeldung:

- in einer Organisation: Anbieter-Schlüssel hinzufügen oder ändern, einem Schlüssel Plattformzugriff geben, Plattform-Schlüssel erstellen, Single Sign-on ändern oder verlangen, Protokolle löschen, die Inhaberschaft übertragen und die Organisation löschen;
- im eigenen Konto: die E-Mail-Adresse ändern, erstmals ein Passwort festlegen, die Zwei-Faktor-Authentifizierung ausschalten, Wiederherstellungscodes neu erzeugen und das Konto löschen.

## Suche und Tastenkürzel

⌘K oder Strg+K öffnet die Suche: zu jeder Seite springen; Schlüssel, Modelle, Mitglieder und Dienstkonten des aktuellen Projekts finden; Neues erstellen; Organisation oder Projekt wechseln; das Design ändern.

Einzelne Tasten wirken, solange kein Eingabefeld den Fokus hat:

| Tasten | Aktion |
| --- | --- |
| `c` | Auf der aktuellen Seite erstellen, etwa einen Schlüssel unter **API-Schlüssel** |
| `/` | Auf dieser Seite suchen |
| `[` | Seitenleiste ein- oder ausklappen |
| `g` `o`, `g` `k`, `g` `l`, `g` `m`, `g` `p` | Zur Übersicht, zu den API-Schlüsseln, Anfrageprotokollen, Modellen oder Einstellungen des Projekts |
| `g` `s`, `g` `a` | Zu den Einstellungen oder zum Audit-Log der Organisation |
| `?` | Alle Tastenkürzel anzeigen |

Eine Liste, etwa der API-Schlüssel oder Mitglieder, ist ein einziger Tab-Stopp. Darin wechseln Pfeil nach oben und unten oder `j` und `k` die Zeile, und Enter öffnet sie; Pfeil nach rechts führt zu den Aktionen einer Zeile, Pfeil nach links oder Escape zurück. Wo sich Zeilen auswählen lassen, etwa unter **Modelle hinzufügen**, wählt Leertaste oder `x` eine Zeile aus, Umschalt mit Pfeil erweitert die Auswahl, ⌘A oder Strg+A wählt alle Zeilen aus und Escape hebt die Auswahl auf. Der erste Tab auf einer Seite erreicht **Zum Inhalt springen**, das die Seitenleiste überspringt.

## Seiten

| Seite | Inhalt |
| --- | --- |
| [Organisationen und Projekte](organizations-and-projects.md) | Anlegen und Wechseln, Übersichten, Einstellungen, Single Sign-on, das Audit-Log |
| [Mitglieder und Rollen](members-and-roles.md) | Einladungen, Rollen und Rechte, Dienstkonten, Modellzugriff |
| [Anbieter-Schlüssel](provider-keys.md) | OpenAI und Anthropic anbinden |
| [Modelle und Aliasse](models-and-aliases.md) | Der Katalog des Projekts, Verfügbarkeit, eigene Werte und Aliasse |
| [API-Schlüssel](api-keys.md) | Schlüssel für Apps und Skripte sowie Plattform-Schlüssel |
| [Limits und Budgets](limits-and-budgets.md) | Raten und Budgets für Projekte, Mitglieder und Schlüssel |
| [Anfrageprotokolle](request-logs.md) | Jede Anfrage, ihr Inhalt, Exporte und Aufbewahrung |
| [Playground](playground.md) | Modelle nebeneinander ausprobieren |
| [Konto und Sprache](account.md) | Profil, Sicherheit, Sitzungen, Design und Sprache |
| [Instanzverwaltung](instance-administration.md) | Benutzer, Organisationen und Einstellungen der Installation |
