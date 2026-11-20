---
type: Story
title: "Phrase de récupération"
description: "Une phrase que seul l'utilisateur connaît, qui protège sa sauvegarde."
tags: [product]
status: draft
state: todo
parent: epics/08-sauvegarde-et-transfert-d-historique.md
depends_on: [stories/02-06-base-locale-chiffree.md]
priority: medium
created: 2026-11-20
---

# Phrase de récupération

Une phrase que seul l'utilisateur connaît, qui protège sa sauvegarde.

## Critères d'acceptation

- Phrase générée sur l'appareil (24 mots d'une liste standard), affichée une fois, vérifiée par l'utilisateur.
- Clé dérivée par Argon2id ; la phrase ne quitte jamais l'appareil.

## Reviews

```workstream
reviews
```
