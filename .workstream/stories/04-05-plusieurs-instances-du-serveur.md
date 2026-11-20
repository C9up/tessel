---
type: Story
title: "Plusieurs instances du serveur"
description: "La livraison marche quand le serveur tourne en plusieurs exemplaires."
tags: [product]
status: draft
state: todo
parent: epics/04-acheminement-des-messages.md
depends_on: [stories/04-03-livraison-en-temps-reel.md]
priority: medium
created: 2026-11-20
---

# Plusieurs instances du serveur

La livraison marche quand le serveur tourne en plusieurs exemplaires.

## Critères d'acceptation

- Diffusion entre instances par `quasar` (pub/sub) ou par le bus que `relay` accepte.
- Test à deux instances : un appareil connecté à l'une reçoit un message envoyé par l'autre.

## Reviews

```workstream
reviews
```
