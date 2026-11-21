---
type: Story
title: "Authentification par la clé d'appareil"
description: "L'appareil prouve qui il est en signant un défi : pas de mot de passe à voler."
tags: [product]
status: draft
state: review
parent: epics/03-comptes-et-appareils.md
depends_on: [stories/03-01-module-identity-et-creation-de-compte.md]
priority: high
created: 2026-11-20
commits: [216d97594a2b86c75c7f414cd3eaceb0b634566f]
---

# Authentification par la clé d'appareil

L'appareil prouve qui il est en signant un défi : pas de mot de passe à voler.

## Critères d'acceptation

- Défi aléatoire signé par la clé d'appareil ; session ouverte par `warden`, liée à cet appareil.
- Un appareil révoqué est refusé, ses sessions tombent.
- Tentatives limitées par `blackhole`.

## Reviews

```workstream
reviews
```
