---
type: Story
title: "Contrat client-serveur avec comet"
description: "Un seul contrat JSON-RPC, partagé par le serveur, le web et le mobile."
tags: [product]
status: draft
state: review
parent: epics/01-fondations-du-depot.md
depends_on: [stories/01-02-serveur-ream-sur-le-modele-nadeo.md]
priority: high
created: 2026-11-20
---

# Contrat client-serveur avec comet

Un seul contrat JSON-RPC, partagé par le serveur, le web et le mobile.

## Critères d'acceptation

- `packages/protocol` déclare les méthodes et leurs types, importés par le serveur et par les clients.
- Le serveur expose ses méthodes par `comet` ; le client `comet` isomorphe tourne tel quel dans le web et dans Expo.
- Une méthode `ping` est testée de bout en bout depuis les deux clients.
- Les erreurs suivent la convention de nommage de Ream (ADR-004).

## Reviews

```workstream
reviews
```
