---
type: Story
title: "SDK de bots sur Node"
description: "Écrire un bot en TypeScript : le cœur Rust dans Node par NAPI, ses clés chez son auteur."
tags: [product]
status: draft
state: todo
parent: epics/22-bots-et-integrations.md
depends_on: [stories/22-04-commandes-et-reponses-interactives.md]
priority: backlog
created: 2026-11-20
---

# SDK de bots sur Node

Écrire un bot en TypeScript : le cœur Rust dans Node par NAPI, ses clés chez son auteur.

## Critères d'acceptation

- Liaison NAPI de `tessel-core` (ADR-003), même API que le web, vérifiée par le test de contrat.
- Paquet TypeScript : se connecter, recevoir les messages, répondre aux commandes ; clés dans une base locale chiffrée sur la machine du bot.
- Un bot d'exemple (echo) dans le dépôt, avec sa documentation.

## Reviews

```workstream
reviews
```
