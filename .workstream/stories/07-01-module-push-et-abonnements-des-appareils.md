---
type: Story
title: "Module push et abonnements des appareils"
description: "Chaque appareil s'abonne, et le serveur le réveille quand une enveloppe l'attend."
tags: [product]
status: draft
state: todo
parent: epics/07-notifications-push.md
depends_on: [stories/04-03-livraison-en-temps-reel.md]
priority: high
created: 2026-11-20
---

# Module push et abonnements des appareils

Chaque appareil s'abonne, et le serveur le réveille quand une enveloppe l'attend.

## Critères d'acceptation

- Abonnement par appareil, supprimé à la révocation de l'appareil.
- Une enveloppe en attente déclenche un réveil (événement du bus, tâche `bay`).
- Les réveils sont regroupés : une rafale de messages ne donne qu'une notification.

## Reviews

```workstream
reviews
```
