---
title: Tools
description: Wählen Sie die MCP-Tools, die das Portal eines Projekts anbietet, wer sie aufruft, und verbinden Sie Clients.
order: 6
---

**Tools** in einem Projekt legt fest, welche MCP-Server und Tools sein Portal anbietet, und verbindet Clients wie claude.ai damit. Sie brauchen `tools.view`, um die Seite zu sehen, `tools.manage`, um sie zu ändern, und `tools.use`, um Tools aufzurufen ([MCP-Portal](../concepts/mcp-portal.md) erklärt die Teile).

## Tools einschalten

1. Öffnen Sie die Seite **Tools** des Projekts. Der Tab **Tools** listet die Tools jedes Servers, mit der Verbindung des Servers und wie viele seiner Tools an sind.
2. Finden Sie ein Tool über seinen Namen oder seine Beschreibung, oder filtern Sie nach **Server**, **Zustand** und **Hinweis**. Hinweise wie **Nur lesend** und **Destruktiv** sind Angaben des Servers, keine Garantien.
3. Schalten Sie ein Tool mit seinem Schalter ein, oder alle Tools eines Servers mit **Alle einschalten** in der Kopfzeile des Servers; **Alle ausschalten** schaltet sie wieder aus. Der Zustand eines Tools wechselt zu **An**, und das Portal bietet es sofort an.

Ein Tool mit **Geändert** wartet darauf, dass ein Admin seine neue Definition unter [MCP-Server der Organisation](mcp-servers.md#ein-geändertes-tool-prüfen) prüft, und lässt sich bis dahin nicht einschalten. Wählen Sie ein Tool, um seine Details, sein Eingabeschema, seine Aufrufe der letzten 7 Tage und **Aufrufe ansehen** zu sehen, das die [Anfrageprotokolle](request-logs.md) gefiltert auf das Tool öffnet.

## Einen Server nutzen

Der Tab **Server** listet die Server, die das Projekt nutzt, mit ihrer Verbindung, dem Anteil eingeschalteter Tools und ihren Aufrufen der letzten 7 Tage.

- **Einen Server nutzen** fügt einen Server der Organisation hinzu. Seine Tools sind zunächst aus. Sie brauchen dafür auch `mcp_servers.view`.
- **Nicht mehr nutzen** nimmt die Tools des Servers aus dem Portal. Nutzen Sie ihn wieder, sind alle Tools aus.

## Einen Client verbinden

Der Tab **Verbinden** oder **Client verbinden** in der Kopfzeile zeigt die Portaladresse, etwa `https://kaeso.example.com/mcp/proj_…`, mit einer Schaltfläche zum Kopieren.

1. Wählen Sie den Client: Claude, ChatGPT, Cursor oder GitHub Copilot. Der Dialog zeigt die Schritte dieses Clients mit der Adresse.
2. Fügen Sie die Adresse im Client hinzu. Der Client öffnet Ihren Browser, in dem Sie sich bei Kaeso anmelden.
3. Prüfen Sie, worum der Client bittet, und wählen Sie dann **Verbinden**. Die Zustimmungsseite nennt jeden Server mit der Zahl seiner Tools, die der Client aufrufen kann, klappt zu den Namen der Tools auf und listet die Server, bei denen Sie noch Ihr eigenes Konto verbinden müssen.

**Verbundene Clients** listet jeden Client, den jemand verbunden hat, wer ihn verbunden hat, wo er seine Anmeldungen empfängt und wann er verbunden und zuletzt genutzt wurde. **Trennen** beendet seine Tokens sofort.

Ihr eigener Agent ruft das Portal mit einem API-Schlüssel auf, der die Fähigkeit **Tools** hat. Schalten Sie sie ein, wenn Sie [einen Schlüssel erstellen](api-keys.md#schlüssel-erstellen).

## Festlegen, wer welches Tool aufruft

Jedes Mitglied, jedes Dienstkonto und jeder Schlüssel darf jedes Tool aufrufen, das das Projekt anbietet, sofern es `tools.use` hat. Um das einzuschränken, öffnen Sie das Mitglied, das Dienstkonto oder den Schlüssel und bearbeiten den Abschnitt **Tools**: Schalten Sie **Jedes Tool, das das Projekt anbietet** aus und wählen Sie die Tools. Die Tools eines Schlüssels liegen immer innerhalb derer seines Inhabers.

## Nächste Schritte

- [MCP-Server der Organisation](mcp-servers.md): Fügen Sie Server hinzu, melden Sie sich bei ihnen an und prüfen Sie geänderte Tools.
- [MCP-Portal-API](../api/mcp-portal.md): Schlagen Sie Anmeldung, Versionen und Fehler des Portals nach.
- [Limits und Budgets](limits-and-budgets.md): Begrenzen Sie die Tool-Aufrufe pro Minute für ein Projekt, ein Mitglied oder einen Schlüssel.
