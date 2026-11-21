---
type: Story
title: "Contrôles automatiques"
description: "Une commande qui vérifie tout, branchée sur la CI de la forge où le dépôt sera publié."
tags: [product]
status: draft
state: done
parent: epics/01-fondations-du-depot.md
depends_on: [stories/01-02-serveur-ream-sur-le-modele-nadeo.md, stories/01-04-squelette-de-tessel-core-et-chaine-de-compilatio.md]
priority: medium
created: 2026-11-20
---

# Contrôles automatiques

Une commande qui vérifie tout, branchée sur la CI de la forge où le dépôt sera publié.

## Critères d'acceptation

- `pnpm check` enchaîne lint, types, tests TS, test d'architecture, `cargo test`, `clippy`, `cargo audit`, `cargo deny`.
- La CI de la forge lance `pnpm check` à chaque push : GitHub Actions, dépôt public `C9up/tessel` (choix du propriétaire, 20.11.2026).
- Le dépôt public ne contient aucun secret (contrôle automatique des fuites de secrets).

## Reviews

```workstream
reviews
```
