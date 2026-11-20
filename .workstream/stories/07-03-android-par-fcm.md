---
type: Story
title: "Android par FCM"
description: "Le téléphone est réveillé par un message FCM sans contenu, et affiche la notification lui-même."
tags: [product]
status: draft
state: todo
parent: epics/07-notifications-push.md
depends_on: [stories/07-01-module-push-et-abonnements-des-appareils.md]
priority: high
created: 2026-11-20
---

# Android par FCM

Le téléphone est réveillé par un message FCM sans contenu, et affiche la notification lui-même.

## Critères d'acceptation

- Dépend du pilote FCM de nova (`ream-dev/packages/nova/.workstream/epics/03-native-push-fcm-apns-unifiedpush.md`).
- Message FCM *data-only*, sans contenu.
- L'app se réveille, récupère, déchiffre et affiche une notification locale.

## Reviews

```workstream
reviews
```
