---
title: Anbieter-Schlüssel
description: Verbinden Sie die Schlüssel Ihrer Organisation für Modellanbieter und wählen Sie, welche Projekte welchen nutzen.
order: 3
---

Anbieter-Schlüssel sind die Schlüssel Ihrer Organisation für die [Modellanbieter, die Kaeso aufruft](../concepts/supported-providers.md). Kaeso speichert sie verschlüsselt mit dem Hauptschlüssel der Installation und zeigt sie nie wieder an. Apps sehen sie nie: Sie rufen Kaeso stattdessen mit [API-Schlüsseln](api-keys.md) auf.

## Schlüssel hinzufügen

**Anbieter** listet unter **Anbieter-Schlüssel** die Anbieter, für die Ihre Organisation Schlüssel hat. Solange sie keinen hat, erklärt die Seite Anbieter-Schlüssel und bietet **Anbieter-Schlüssel hinzufügen** an. Ab mehr als acht Schlüsseln findet eine Suche über der Liste sie nach Anbieter, Schlüssel oder Projektname. Daneben listet **Eigene Anbieter** die Server, die Ihre Organisation selbst hinzugefügt hat ([Eigene Anbieter](custom-providers.md)).

1. Wählen Sie unter **Anbieter** **Anbieter-Schlüssel hinzufügen**.
2. **Für welchen Anbieter ist der Schlüssel?** Suchen Sie den Anbieter und wählen Sie ihn. Anbieter, die schon Schlüssel haben, stehen oben, mit der Anzahl ihrer Schlüssel.
3. Fügen Sie den Schlüssel ein und geben Sie ihm einen Namen, etwa `Production`. Der Dialog verlinkt die Seite des Anbieters, auf der Sie Schlüssel erstellen; [Unterstützte Anbieter](../concepts/supported-providers.md) nennt sie ebenfalls.
4. Hat der Anbieter schon einen Standardschlüssel, wählen Sie, ob der neue Schlüssel zum Standard wird.

Jede Aktion an Anbieter-Schlüsseln braucht ihr eigenes Recht:

| Aktion | Recht |
| --- | --- |
| Schlüssel hinzufügen | **Anbieter-Schlüssel hinzufügen** |
| Schlüssel ersetzen, umbenennen, zuweisen oder zum Standard machen | **Anbieter-Schlüssel ändern** |
| Schlüssel löschen | **Anbieter-Schlüssel löschen** |

Jede Aktion außer dem Löschen erfordert zusätzlich eine kürzliche Anmeldung.

## Dienstkonto-Schlüssel von Google Cloud hinzufügen

Für Gemini Enterprise Agent Platform ist ein Anbieter-Schlüssel entweder ein API-Schlüssel für den Express-Modus oder der JSON-Schlüssel eines Dienstkontos von Google Cloud. Ein Schlüssel für den Express-Modus erreicht nur die Gemini-Modelle, die der Express-Modus führt. Ein Dienstkonto-Schlüssel erreicht jedes Modell, das die Plattform seinem Projekt anbietet.

1. Legen Sie in der Google Cloud Console unter **IAM & Admin → Service accounts** ein Dienstkonto im Projekt an, das die Anfragen bezahlen soll. Geben Sie ihm die Rolle **Agent Platform User** (`roles/aiplatform.user`), die kleinste Rolle, die Modelle aufruft ([Zugriffssteuerung](https://docs.cloud.google.com/gemini-enterprise-agent-platform/machine-learning/general/access-control)).
2. Öffnen Sie das Dienstkonto, dann **Keys → Add key → Create new key → JSON**. Google lädt die Schlüsseldatei herunter. Organisationen, die seit Mai 2024 angelegt wurden, sperren das Anlegen von Schlüsseln standardmäßig mit der Richtlinie `iam.disableServiceAccountKeyCreation`; ein Administrator für Organisationsrichtlinien kann es für das Projekt erlauben ([Dienstkonto-Schlüssel anlegen und löschen](https://docs.cloud.google.com/iam/docs/keys-create-delete)).
3. Wählen Sie in Kaeso unter **Anbieter** **Anbieter-Schlüssel hinzufügen**, dann **Gemini Enterprise Agent Platform** und dann **Dienstkonto-Schlüssel**.
4. Fügen Sie den Inhalt der Schlüsseldatei ein oder wählen Sie die Datei, wählen Sie einen **Standort** und geben Sie dem Schlüssel einen Namen. `global` bietet die meisten Modelle; wählen Sie eine Region oder `us` oder `eu`, wenn die Anfragen dort bleiben müssen.

Kaeso prüft, dass die Datei ein Dienstkonto-Schlüssel ist, dessen `token_uri` `https://oauth2.googleapis.com/token` ist, und lehnt sie sonst ab. Es speichert die ganze Datei verschlüsselt wie jeden anderen Schlüssel und zeigt sie nie wieder; die Liste zeigt die E-Mail-Adresse des Dienstkontos, sein Projekt, den Standort und die letzten vier Zeichen der Schlüssel-ID. Für jede Anfrage signiert Kaeso mit dem Schlüssel eine Token-Anfrage, tauscht sie bei Google gegen ein Zugriffstoken, das eine Stunde gilt, und behält das Token bis fünf Minuten vor seinem Ablauf im Speicher. Lehnt Google den Schlüssel ab, erhalten Apps einen abgelehnten Anbieter-Schlüssel.

Um einen anderen Standort aufzurufen, wählen Sie im Menü des Schlüssels **Bearbeiten**. Um eine neue Schlüsseldatei desselben Dienstkontos zu verwenden, wählen Sie **Ersetzen**. Löschen Sie die heruntergeladene Datei, sobald Kaeso den Schlüssel hat.

## Schlüssel für Cloudflare AI Gateway

Für Cloudflare AI Gateway ist ein Anbieter-Schlüssel ein Cloudflare-API-Token, und er nennt das Konto, das er aufruft, und optional das Gateway, über das seine Anfragen laufen.

1. Legen Sie im Cloudflare-Dashboard unter **My Profile → API Tokens** oder unter **Manage account → Account API Tokens** des Kontos ein Token mit der Berechtigung **Account → Workers AI → Read** für das Konto an. Jede Modellanfrage braucht sie, auch für Modelle anderer Anbieter ([REST API](https://developers.cloudflare.com/ai-gateway/usage/rest-api/#authentication)); ein Token mit nur AI-Gateway-Berechtigungen wird abgelehnt.
2. Kopieren Sie die Konto-ID von der Startseite des Kontos oder aus der Adresse des Dashboards ([Konto- und Zonen-IDs finden](https://developers.cloudflare.com/fundamentals/account/find-account-and-zone-ids/)).
3. Wählen Sie in Kaeso unter **Anbieter** **Anbieter-Schlüssel hinzufügen**, dann **Cloudflare AI Gateway**, und fügen Sie das Token und die Konto-ID ein.
4. Geben Sie optional ein **Gateway** an: die ID eines Gateways des Kontos. Die Anfragen laufen dann darüber, mit seinen Logs, seinem Caching, seinen Ratenlimits und Guardrails, und können seine Aliasse (`custom/…`) und dynamischen Routen (`dynamic/…`) aufrufen. Ohne Gateway laufen sie über das Gateway `default` des Kontos, das Cloudflare bei der ersten Anfrage anlegt.

Modelle anderer Anbieter werden mit den Unified-Billing-Guthaben des Kontos bezahlt oder mit den Anbieter-Schlüsseln, die auf dem Gateway unter dem Alias `default` gespeichert sind ([eigene Schlüssel](https://developers.cloudflare.com/ai-gateway/configuration/bring-your-own-keys/)). Ohne beides antwortet Cloudflare mit leerem Guthaben, das Apps als aus Abrechnungsgründen abgelehnten Anbieter-Schlüssel erhalten. Workers-AI-Modelle werden über Workers AI abgerechnet, auch wenn das Konto kein Guthaben hat.

Die Liste zeigt die Konto-ID und das Gateway neben dem Schlüssel. Um den Schlüssel auf ein anderes Konto oder Gateway umzustellen, wählen Sie **Bearbeiten** in seinem Menü.

## Mit einem ChatGPT-Konto anmelden

Für OpenAI kann ein Anbieter-Schlüssel auch ein ChatGPT-Konto sein. Seine Anfragen nutzen dann das ChatGPT-Abo des Kontos statt eines OpenAI-API-Kontos, im Rahmen der Limits des Abos.

> [!WARNING]
> Nutzen Sie nur Ihr eigenes Konto; es zu teilen verstößt gegen die Bedingungen von OpenAI.

1. Wählen Sie unter **Anbieter** **Anbieter-Schlüssel hinzufügen** und dann **OpenAI**.
2. Wählen Sie **ChatGPT-Konto**. Hat OpenAI schon einen Standardschlüssel, wählen Sie, ob das Konto zum Standard wird.
3. Geben Sie dem Schlüssel einen Namen und wählen Sie **Mit ChatGPT fortfahren**. ChatGPT öffnet sich in einem neuen Tab: Melden Sie sich an und stimmen Sie Kaeso zu.
4. Der Tab wechselt dann zu einer Adresse, die mit `http://127.0.0.1` beginnt und nicht laden kann. Das ist so gewollt. Kopieren Sie diese Adresse aus der Adressleiste.
5. Fügen Sie sie in **Adresse der Seite** ein und wählen Sie **Konto hinzufügen**.

Schließen Sie innerhalb von zehn Minuten ab, als die Person, die begonnen hat. Kaeso prüft die Adresse, meldet das Konto bei OpenAI an und speichert seine Tokens verschlüsselt wie jeden anderen Schlüssel. Es erneuert sie selbst, bevor sie ablaufen.

Ein ChatGPT-Konto bedient Anfragen nach den Regeln von OpenAI für die Nutzung von Abos:

- Jede Anfrage geht als Stream, den OpenAI nicht speichert, an `https://api.openai.com/v1/responses`, unabhängig von der Basis-URL, die die Installation für OpenAI festlegt. Für Apps, die eine einzelne Antwort erwarten, fügt Kaeso den Stream zu einer Antwort zusammen.
- Kaeso lässt die Felder weg, die ein Abo ablehnt: `temperature`, `top_p`, die Limits für Ausgabe und Tool-Aufrufe, `metadata`, `truncation`, Benutzer-IDs und die anderen [Einschränkungen der Vorschau](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).
- Bilder, Sprachausgabe, Transkriptionen, Embeddings und Token-Zählungen brauchen einen API-Schlüssel; für das Konto lehnt Kaeso sie ab, bevor etwas gesendet wird, und der Fehler sagt das.
- Ist das Nutzungslimit des Abos erreicht, erhalten Apps eine `429`, die auf die [Nutzungseinstellungen von ChatGPT](https://chatgpt.com/settings/usage) verweist.
- **Verfügbarkeit prüfen** bietet nur die Modelle an, die das Konto für sein Abo listet.

## Mit einem Grok-Konto anmelden

Für SpaceX kann ein Anbieter-Schlüssel auch ein Grok-Konto sein. Seine Anfragen nutzen dann das Grok-Abo des Kontos statt eines xAI-API-Kontos, sofern das Abo API-Zugang enthält.

> [!WARNING]
> Grok-Konten sind experimentell: xAI kann ändern, wie seine Anmeldung funktioniert. Nutzen Sie nur Ihr eigenes Konto; es zu teilen verstößt gegen die Bedingungen von SpaceX.

1. Wählen Sie unter **Anbieter** **Anbieter-Schlüssel hinzufügen** und dann **SpaceX**.
2. Wählen Sie **Grok-Konto**, markiert als **Experimentell**. Hat SpaceX schon einen Standardschlüssel, wählen Sie, ob das Konto zum Standard wird.
3. Geben Sie dem Schlüssel einen Namen und wählen Sie **Anmeldecode holen**. Der Dialog zeigt einen Code, einen Link zur Anmeldeseite von xAI und einen QR-Code dieses Links.
4. Öffnen Sie die Seite auf einem beliebigen Gerät, melden Sie sich bei Grok an und geben Sie den Code ein.
5. Kaeso fügt den Schlüssel selbst hinzu, sobald Sie zustimmen. Lehnen Sie ab oder läuft der Code ab, wählen Sie **Neuen Code holen**.

Schließen Sie innerhalb von zehn Minuten ab, als die Person, die begonnen hat. Kaeso speichert die Tokens des Kontos verschlüsselt wie jeden anderen Schlüssel und erneuert sie selbst in den letzten zwei Minuten, bevor sie ablaufen.

Ein Grok-Konto bedient Anfragen wie ein xAI-API-Schlüssel, unter `https://api.x.ai/v1` mit dem Token des Kontos, unabhängig von der Basis-URL, die die Installation für SpaceX festlegt. Lehnt xAI das Konto ab, weil sein Abo keinen API-Zugang hat, zeigt der Schlüssel **Kein API-Zugang** und seine Anfragen schlagen fehl.

## Schlüssel eines Projekts wählen

Jeder Anbieter hat einen Standardschlüssel, den jedes Projekt ohne zugewiesenen Schlüssel nutzt. Der erste Schlüssel, den Sie für einen Anbieter hinzufügen, wird sein Standard. Um den Standard zu verlegen, wählen Sie **Als Standard festlegen** im Menü eines anderen Schlüssels.

Um einigen Projekten einen anderen Schlüssel zu geben, wählen Sie **Projekten zuweisen** im Menü dieses Schlüssels und wählen Sie die Projekte. Sie nutzen dann für diesen Anbieter diesen Schlüssel statt des Standards. Ein Projekt nutzt je Anbieter einen Schlüssel.

## Schlüssel ersetzen, umbenennen oder löschen

Die Liste zeigt zu jedem Schlüssel seinen Namen, die letzten vier Zeichen, die Projekte, die ihn nutzen, wer ihn hinzugefügt hat und wann er zuletzt genutzt wurde. Ein Konto zeigt statt der letzten vier Zeichen seine E-Mail-Adresse und sein Produkt, etwa **ChatGPT** oder **Grok**; ein Grok-Konto ist zusätzlich als **Experimentell** markiert. Das Menü eines Schlüssels hat diese Aktionen:

| Aktion | Wirkung |
| --- | --- |
| Ersetzen | Nimmt ein neues Secret für denselben Schlüssel an, etwa nachdem Sie ihn beim Anbieter rotiert haben; bei einem Dienstkonto-Schlüssel eine neue JSON-Schlüsseldatei |
| Erneut anmelden | Für ein Konto: meldet dasselbe Konto erneut an, etwa nachdem es **Abgemeldet** oder **Kein API-Zugang** zeigt, und beendet seine vorherige Sitzung bei OpenAI oder xAI |
| Nutzung verwalten | Für ein ChatGPT-Konto: öffnet die Nutzungseinstellungen von ChatGPT, in denen sein Inhaber prüft und begrenzt, was Kaeso nutzt |
| Umbenennen | Ändert den Namen, den die Konsole zeigt. Bei einem Dienstkonto-Schlüssel heißt es **Bearbeiten** und ändert auch den Standort, bei einem Schlüssel für Cloudflare AI Gateway das Konto und das Gateway |
| Löschen | Entfernt den Schlüssel. Seine Projekte nutzen wieder den Standard. Bei einem Konto beendet Kaeso außerdem seine Sitzung bei OpenAI oder xAI |

Ein Konto zeigt **Abgemeldet**, wenn der Anbieter seine Sitzung beendet hat, etwa nachdem sein Inhaber Kaeso in ChatGPT getrennt hat. Ein Grok-Konto zeigt **Kein API-Zugang**, wenn xAI sein Abo ablehnt. In beiden Fällen schlagen seine Anfragen fehl, bis jemand mit **Anbieter-Schlüssel ändern** **Erneut anmelden** wählt.

Lehnt ein Anbieter einen Schlüssel dreimal nacheinander ab oder beendet er die Sitzung eines Kontos, schickt Kaeso allen mit **Anbieter-Schlüssel ändern** eine E-Mail. Sie nennt den Anbieter, den Schlüssel mit seinen letzten vier Zeichen oder das Konto, die Projekte, die ihn verwenden, die Antwort des Anbieters, die Zeitpunkte der Ablehnungen und die letzte Verwendung, damit die Lesenden entscheiden können, ob sie ihn ersetzen, das Konto aufladen oder sich erneut anmelden ([E-Mails wählen](account.md#e-mails-wählen)).

Wenn Sie den Standard löschen, wird der zuletzt hinzugefügte andere Schlüssel des Anbieters zum Standard. Gibt es keinen, schlagen Anfragen an diesen Anbieter fehl. Der Dialog sagt Ihnen, was zutrifft.

> [!NOTE]
> Der Schlüssel eines Projekts bestimmt, welche Modelle der Anbieter ihm bereitstellt. Wenn Sie einen Schlüssel hinzufügen, ersetzen, zuweisen, löschen oder zum Standard machen, setzt Kaeso die [Verfügbarkeit](models-and-aliases.md#verfügbarkeit) der Modelle dieses Anbieters in jedem Projekt auf **Nicht geprüft** zurück.

## Nächste Schritte

- [Modelle und Aliasse](models-and-aliases.md): Fügen Sie die Modelle hinzu, die ein Projekt aufrufen darf.
- [API-Schlüssel](api-keys.md): Erstellen Sie die Schlüssel, mit denen Ihre Apps Kaeso aufrufen.
