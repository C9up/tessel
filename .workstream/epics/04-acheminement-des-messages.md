---
type: Epic
title: "Acheminement des messages"
description: "Le facteur aveugle : recevoir, ranger et livrer des enveloppes chiffrées à chaque appareil, en direct ou à la reconnexion."
tags: [product]
status: draft
order: 4
created: 2026-11-20
---

# Acheminement des messages

Le facteur aveugle : recevoir, ranger et livrer des enveloppes chiffrées à chaque appareil, en direct ou à la reconnexion.

## Périmètre

- Module serveur `delivery` : boîtes aux lettres, ordre par groupe, accusés de remise, rétention et purge.
- Temps réel par `relay` (SSE pour recevoir, POST `comet` pour envoyer), diffusion multi-instance par `quasar`.
- Limitation de débit et tailles maximales.

## Terminé quand

- Un message envoyé arrive en direct sur chaque appareil connecté, et à la reconnexion sur les autres, sans perte ni doublon.
- Le serveur ne lit que l'en-tête public MLS (groupe, epoch, type), jamais le contenu.

## Dépendances

- Epic 03 (appareils authentifiés).
- Plus tard : le WebSocket de relay, story `ream-dev/packages/relay/.workstream/stories/01-05-transport-websocket-pour-relay.md`.

## Stories

```workstream
stories
```
