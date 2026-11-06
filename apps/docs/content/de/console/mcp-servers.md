---
title: MCP-Server der Organisation
description: Fügen Sie die MCP-Server der Organisation hinzu, melden Sie sich an und prüfen Sie die Tools, die sie ändern.
order: 4
---

**MCP-Server** listet die Server, aus denen die Projekte Ihrer Organisation Tools beziehen können. Jede Zeile zeigt die Adresse des Servers, sein Kürzel, wie Kaeso verbunden ist, seine Tools und die Projekte, die ihn nutzen. Sie brauchen `mcp_servers.view`, um die Seite zu sehen, und `mcp_servers.manage`, um sie zu ändern ([MCP-Portal](../concepts/mcp-portal.md) erklärt die Teile).

## Einen Server hinzufügen

1. Öffnen Sie **MCP-Server** und wählen Sie **Server hinzufügen**.
2. Geben Sie die Streamable-HTTP-Adresse des Servers ein, etwa `https://mcp.example.com/mcp`, und wählen Sie **Adresse prüfen**. Kaeso zeigt, als was sich der Server ausgibt, seine MCP-Version, wie er sich anmeldet und wie viele Tools er listet.
3. Behalten oder ändern Sie Name und Kürzel. Das Kürzel steht für Clients vor den Tools des Servers, etwa `x__create_post`.
4. Wählen Sie, wie sich Kaeso anmeldet: **Ohne Anmeldung**, **Geheimer Header** oder **OAuth**, und ob das Konto **Gemeinsam** ist oder **Jede Person meldet sich an**.
5. Für einen gemeinsamen geheimen Header geben Sie Name und Wert des Headers ein. Für gemeinsames OAuth wählen Sie **Bei … anmelden**: Die Anmeldung öffnet sich in einem neuen Tab, und der Dialog zeigt **Verbunden als** mit dem Konto, sobald Sie zurück sind.
6. Wählen Sie die Tools, die Projekte anbieten sollen. Schreibgeschützte Tools sind gewählt, destruktive sind markiert. Lassen Sie **Geänderte Tools zur Prüfung zurückhalten** an, wenn Sie den Server nicht selbst betreiben.
7. Wählen Sie die Projekte, die den Server nutzen, und wählen Sie **Server hinzufügen**. Die gewählten Tools sind in diesen Projekten an.

Eine Adresse in einem privaten Netz funktioniert nur, wenn die Installation das Netz in `mcp_servers.allowed_networks` erlaubt ([Konfiguration](../self-hosting/configuration.md)). Einfaches `http` funktioniert nur in diesen Netzen.

Nimmt der Autorisierungsserver des Servers weder Client-Metadatendokumente noch Registrierungen an, fragt der Dialog nach einer Client-ID und, bei einem vertraulichen Client, nach seinem Geheimnis. Registrieren Sie Kaeso dort mit der Weiterleitungs-URI `https://kaeso.example.com/oauth/upstream/callback`.

## Erneut anmelden oder das Geheimnis ersetzen

Wählen Sie einen Server, um sein Sheet zu öffnen. Unter **Verbindung** startet **Erneut anmelden** eine neue OAuth-Anmeldung, und **Geheimnis ersetzen** setzt einen neuen Header. Ein Server, der die Anmeldung von Kaeso ablehnt, zeigt **Abgemeldet**, bis sich jemand anmeldet.

Bei einem Server, bei dem sich jede Person anmeldet, listet das Sheet keine Verbindung: Jede Person verbindet ihr eigenes Konto unter **Konto** › **Verbindungen**. Die Tools des Servers erscheinen, sobald sich die erste Person verbunden hat.

## Ein geändertes Tool prüfen

Ändert ein Server ein Tool, das zur Prüfung zurückgehalten wird, zeigt das Sheet, wie viele Tools warten, und das Tool zeigt **Geändert** mit **Prüfen**. Die Prüfung zeigt, was sich an Titel, Beschreibung, Ein- und Ausgabeschema und Hinweisen geändert hat, mit dem Hash der freigegebenen und der aktuellen Definition.

- **Änderung freigeben** gibt die geprüfte Definition frei. Projekte, in denen das Tool an ist, bieten es wieder an.
- **Aus lassen** hält das Tool zurück und schaltet es in jedem Projekt aus.

Ändert der Server das Tool erneut, bevor Sie freigeben, wird nichts freigegeben, und Sie prüfen die neue Änderung.

## Einen Server ändern

- **Auf neue Tools prüfen** fragt den Server jetzt nach seinen Tools, statt bis zur nächsten Prüfung zu warten.
- **Projekte wählen** legt die Projekte fest, die den Server nutzen. Jedes Projekt wählt weiterhin seine eigenen Tools.
- **Bearbeiten** benennt den Server um, ändert sein Kürzel oder schaltet **Geänderte Tools zur Prüfung zurückhalten** ein oder aus. Ein neues Kürzel benennt jedes Tool für Clients um.
- **Löschen** entfernt den Server mit seinen Anmeldungen und Tools. Das Portal jedes Projekts bietet sie nicht mehr an.

## Nächste Schritte

- [Tools](tools.md): Schalten Sie die Tools eines Servers in einem Projekt ein und verbinden Sie Clients.
- [MCP-Portal](../concepts/mcp-portal.md): Erfahren Sie, wie Verbindungen, Prüfungen und Tool-Zugriff zusammenspielen.
