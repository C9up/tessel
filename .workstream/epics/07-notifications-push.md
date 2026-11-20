---
type: Epic
title: "Notifications push"
description: "Réveiller les appareils sans rien révéler : la notification ne porte aucun contenu, l'appareil déchiffre et affiche lui-même."
tags: [product]
status: draft
order: 7
created: 2026-11-20
---

# Notifications push

Réveiller les appareils sans rien révéler : la notification ne porte aucun contenu, l'appareil déchiffre et affiche lui-même.

## Périmètre

- Module serveur `push` : abonnements par appareil, réveil sur arrivée d'enveloppe, regroupement.
- Web Push par `nova` ; Android par FCM, via l'extension de `nova` au push natif.
- Préférences de notification par conversation et par espace.

## Terminé quand

- Le web et Android reçoivent une notification d'un message alors que l'app est fermée.
- La charge envoyée à Google ou au service de push ne contient ni texte, ni nom, ni identifiant de conversation.

## Dépendances

- Epic 04.
- Push natif dans nova : epic `ream-dev/packages/nova/.workstream/epics/03-native-push-fcm-apns-unifiedpush.md`.

## Stories

```workstream
stories
```
