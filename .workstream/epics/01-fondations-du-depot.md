---
type: Epic
title: "Fondations du dépôt"
description: "Le dépôt, les apps vides et les outils qui permettent à toutes les autres epics de démarrer."
tags: [product]
status: draft
order: 1
created: 2026-11-20
---

# Fondations du dépôt

Le dépôt, les apps vides et les outils qui permettent à toutes les autres epics de démarrer.

## Périmètre

- Dépôt git et monorepo pnpm + turbo : `apps/server` (Ream, qui sert aussi l'interface aurora / nebula), `crates/tessel-core` (Rust), `packages/protocol` (contrat `comet` partagé).
- Serveur Ream sur le modèle `nadeo` : modules hexagonaux, test d'architecture, outbox, bus d'événements.
- Environnement de développement : Postgres (derrière BlackGate), KeyDB, stockage compatible S3.
- Contrôles automatiques (lint, types, tests TS et Rust, audit des dépendances).
- Décisions d'architecture de départ dans `.workstream/decisions/`.

## Terminé quand

- Chaque app démarre vide et répond (serveur `/health`, page web).
- `tessel-core` est appelé depuis le web (WASM). Le mobile suit la stack mobile de Ream (ADR-003, epic 13).
- `pnpm check` passe : lint, types, tests, test d'architecture, `cargo clippy`, `cargo audit`.
- Les décisions de départ sont écrites.

## Dépendances

- 

## Stories

```workstream
stories
```
