---
type: Review
title: "La déduplication concurrente n’est pas garantie"
description: "La déduplication concurrente n’est pas garantie"
tags: [review]
status: draft
state: todo
parent: stories/04-01-boites-aux-lettres-des-appareils.md
review: rework
created: 2026-11-21
---

# La déduplication concurrente n’est pas garantie

# La déduplication concurrente n’est pas garantie

Le bug 002 décrit une course dans `SqlMailboxes.#storeInGroup` : deux requêtes simultanées du même appareil avec le même `SendId` peuvent toutes deux ne rien voir dans `existing`; la seconde peut consommer une séquence puis être rendue comme `staleEpoch` au lieu de recevoir le même accusé. Le critère de rangement unique et d’accusé rejoué doit couvrir ce cas concurrent.

## Correction

```workstream
commits
```

- 
