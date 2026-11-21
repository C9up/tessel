---
type: Story
title: "Dépôt git et monorepo"
description: "Un dépôt git et un monorepo pnpm + turbo qui accueillent le serveur, le web, le mobile, le cœur Rust et le contrat partagé."
tags: [product]
status: draft
state: done
parent: epics/01-fondations-du-depot.md
priority: high
created: 2026-11-20
---

# Dépôt git et monorepo

Un dépôt git et un monorepo pnpm + turbo qui accueillent le serveur, le web, le mobile, le cœur Rust et le contrat partagé.

## Critères d'acceptation

- `git init` sur la branche `main` ; `.gitignore` qui n'exclut pas `.workstream/` (seulement `.workstream/.local/`).
- Workspace pnpm + turbo, sur le modèle de `liste_course` : `apps/server`, `apps/web`, `apps/mobile`, `crates/tessel-core`, `packages/protocol`.
- Scripts racine `dev`, `build`, `test`, `lint`, `typecheck` et `check` (enchaîne tous les contrôles).
- Workspace Cargo à la racine pour `crates/*`.
- README court : ce qu'est Tessel, lien vers la vision, comment lancer le projet.

## Reviews

```workstream
reviews
```
