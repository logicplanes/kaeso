---
title: Anbieter-Schlüssel
description: OpenAI- und Anthropic-Schlüssel mit einer Organisation verbinden.
order: 2
---

Anbieter-Schlüssel gehören der Organisation. Jedes Projekt nutzt den Standardschlüssel der Organisation für einen Anbieter, sofern ihm kein anderer zugewiesen ist.

## Schlüssel hinzufügen

Auf der Seite **Anbieter** der Organisation **Schlüssel hinzufügen** wählen. Die Anfragen eines Schlüssels zählen gegen die [Ratenlimits](keys-and-limits.md#ratenlimits) des Schlüssels, der sie gesendet hat.

## Schlüssel Projekten zuweisen

Einen Schlüssel den Projekten zuweisen, die ihn nutzen sollen; die anderen behalten den Standard.

```mermaid
flowchart TD
  accTitle: Welchen Provider-Schlüssel eine Anfrage nutzt
  accDescr: Eine Anfrage nutzt den Schlüssel, der ihrem Projekt zugewiesen ist, sonst den Standardschlüssel der Organisation für den Provider.
  request[Anfrage in einem Projekt] --> assigned{Schlüssel dem Projekt zugewiesen?}
  assigned -- ja --> own[Der zugewiesene Schlüssel]
  assigned -- nein --> fallback[Der Standardschlüssel der Organisation]
```
