---
type: Story
title: "Web Push"
description: "Le navigateur reçoit un réveil vide, puis le service worker récupère, déchiffre et affiche."
tags: [product]
status: draft
state: todo
parent: epics/07-notifications-push.md
depends_on: [stories/07-01-module-push-et-abonnements-des-appareils.md]
priority: high
created: 2026-11-20
---

# Web Push

Le navigateur reçoit un réveil vide, puis le service worker récupère, déchiffre et affiche.

## Critères d'acceptation

- `nova` configuré (clés VAPID, route d'abonnement, service worker).
- La charge est vide ou un identifiant opaque à usage unique.
- Le service worker récupère les enveloppes, les déchiffre avec `tessel-core` et affiche la notification.

## Reviews

```workstream
reviews
```
