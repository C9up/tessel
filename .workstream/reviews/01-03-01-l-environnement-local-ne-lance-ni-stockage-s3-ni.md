---
type: Review
title: "L’environnement local ne lance ni stockage S3 ni client web"
description: "L’environnement local ne lance ni stockage S3 ni client web"
tags: [review]
status: draft
state: todo
parent: stories/01-03-environnement-de-developpement.md
review: rework
created: 2026-11-21
---

# L’environnement local ne lance ni stockage S3 ni client web

# L’environnement local ne lance ni stockage S3 ni client web

`compose.yaml` et `scripts/stack.sh` ne lancent que PostgreSQL et KeyDB : aucun stockage compatible S3 n’est présent. De plus, `pnpm dev` ne lance actuellement que les paquets dotés d’un script `dev`; il n’existe ni coquille web ni script web à lancer. Deux critères d’acceptation restent donc à réaliser ou à déplacer explicitement vers des stories ultérieures.

## Correction

```workstream
commits
```

- 
