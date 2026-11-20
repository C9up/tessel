---
type: Story
title: "Serveur Ream sur le modèle nadeo"
description: "Une app Ream vide mais déjà construite comme nadeo : modules hexagonaux, test d'architecture, outbox et bus d'événements."
tags: [product]
status: draft
state: review
parent: epics/01-fondations-du-depot.md
depends_on: [stories/01-01-depot-git-et-monorepo.md]
priority: high
created: 2026-11-20
---

# Serveur Ream sur le modèle nadeo

Une app Ream vide mais déjà construite comme nadeo : modules hexagonaux, test d'architecture, outbox et bus d'événements.

## Critères d'acceptation

- Avant la première ligne : lecture de `nadeo/AGENTS.md`, de sa décision « le métier au centre » et d'un module complet (`site/`).
- `apps/server` créé avec `app/modules/<domaine>/{domain,application,infrastructure,http}` et `start/<module>.ts`.
- Test d'architecture : `domain` n'importe ni `infrastructure`, ni `http`, ni un paquet d'infrastructure.
- Outbox transactionnelle et bus `@c9up/ream/events` branchés ; un événement d'exemple traverse l'outbox dans un test.
- `GET /health` répond ; tests sous `@c9up/helix` + `helix-plugin-ream`.

## Reviews

```workstream
reviews
```
