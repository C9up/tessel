---
type: Review
title: "Un doublon concurrent peut perturber l’ordre du groupe"
description: "Un doublon concurrent peut perturber l’ordre du groupe"
tags: [review]
status: draft
state: todo
parent: stories/04-02-ordre-des-commits-par-groupe.md
review: rework
created: 2026-11-21
---

# Un doublon concurrent peut perturber l’ordre du groupe

# Un doublon concurrent peut perturber l’ordre du groupe

Le bug 002 touche aussi l’ordre : `placed` peut avancer `next_seq` avant que l’`INSERT` de l’enveloppe perde son conflit de déduplication. Il faut rendre déduplication et attribution de séquence atomiques, puis ajouter un test concurrent avec le même `SendId`, afin qu’un retry ne crée ni trou ni faux `staleEpoch`.

## Correction

```workstream
commits
```

- 
