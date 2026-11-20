---
type: Story
title: "Environnement de développement"
description: "Tout ce qu'il faut pour lancer Tessel en local d'une seule commande."
tags: [product]
status: draft
state: review
parent: epics/01-fondations-du-depot.md
depends_on: [stories/01-02-serveur-ream-sur-le-modele-nadeo.md]
priority: high
created: 2026-11-20
---

# Environnement de développement

Tout ce qu'il faut pour lancer Tessel en local d'une seule commande.

## Critères d'acceptation

- Compose de développement : Postgres en TLS mutuel (`scripts/stack.sh`, sans BlackGate depuis l'ADR-006), KeyDB, stockage compatible S3.
- Ports choisis libres par rapport aux autres apps Ream de la machine, et notés dans le README.
- `.env.example` complet, sans secret réel.
- `pnpm dev` lance le serveur et le web ; l'app Expo se lance à part sur émulateur.

## Reviews

```workstream
reviews
```
