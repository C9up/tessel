---
type: Story
title: "Coquille mobile sur la stack Ream"
description: "L'app Tessel démarre sur Android par l'hôte natif de Ream, charge le cœur et parle au serveur : Tessel devient l'app pilote de l'epic 73."
tags: [product]
status: draft
state: todo
parent: epics/13-app-android.md
depends_on: [stories/02-13-liaison-mobile-du-coeur-sur-la-stack-ream.md, stories/01-05-contrat-client-serveur-avec-comet.md]
priority: high
created: 2026-11-20
---

# Coquille mobile sur la stack Ream

L'app Tessel démarre sur Android par l'hôte natif de Ream, charge le cœur et parle au serveur : Tessel devient l'app pilote de l'epic 73.

## Critères d'acceptation

- L'app tourne sur la stack mobile de Ream (`ream:epics/73-mobile-apps-on-the-ream-ecosystem-without-react.md`), sans React ni Expo dans ses dépendances.
- Le cœur est chargé par sa liaison mobile (02-13) ; la clé de la base locale est protégée par le Keystore d'Android.
- Le client `@tessel/protocol` (comet) répond à `system.ping` depuis le téléphone.
- Chaque manque de la stack rencontré en route est un ticket dans le Workstream de Ream (racine pour l'epic 73, ou le paquet concerné).

## Reviews

```workstream
reviews
```
