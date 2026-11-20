---
type: Story
title: "Coquille web et worker crypto"
description: "L'app web vide, sûre par construction."
tags: [product]
status: draft
state: todo
parent: epics/12-client-web.md
depends_on: [stories/01-04-squelette-de-tessel-core-et-chaine-de-compilatio.md, stories/02-10-liaisons-wasm-pour-le-web.md]
priority: high
created: 2026-11-20
---

# Coquille web et worker crypto

L'app web vide, sûre par construction.

## Critères d'acceptation

- App Aurora / Nebula en atomic design, sur le modèle de l'epic 28 de nadeo (composants Nebula, jamais d'élément natif à la place).
- Le rendu serveur ne produit que la coquille ; aucune donnée d'utilisateur côté serveur.
- `tessel-core` chargé dans un Web Worker ; CSP stricte (pas de script inline, `wasm-unsafe-eval` seulement).

## Reviews

```workstream
reviews
```
