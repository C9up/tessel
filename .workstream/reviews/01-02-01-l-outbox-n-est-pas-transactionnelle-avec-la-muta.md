---
type: Review
title: "L’outbox n’est pas transactionnelle avec la mutation métier"
description: "L’outbox n’est pas transactionnelle avec la mutation métier"
tags: [review]
status: draft
state: todo
parent: stories/01-02-serveur-ream-sur-le-modele-nadeo.md
review: rework
created: 2026-11-21
---

# L’outbox n’est pas transactionnelle avec la mutation métier

# L’outbox n’est pas transactionnelle avec la mutation métier

Le bug 001 montre que l’écriture métier et la ligne d’outbox sont deux opérations séparées. `enqueueForDurability` intercepte même l’échec d’insertion et poursuit. Une interruption après la mutation, ou une panne lors de l’insertion, perd donc définitivement l’événement. Le critère « outbox transactionnelle » n’est pas rempli : il faut partager la transaction de la mutation et refuser de valider si l’outbox n’est pas durable.

## Correction

```workstream
commits
```

- 
