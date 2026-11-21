---
type: Review
title: "Les derniers accusés concurrents peuvent laisser le contenu stocké"
description: "Les derniers accusés concurrents peuvent laisser le contenu stocké"
tags: [review]
status: draft
state: todo
parent: stories/04-04-accuses-de-remise-et-purge.md
review: rework
created: 2026-11-21
---

# Les derniers accusés concurrents peuvent laisser le contenu stocké

# Les derniers accusés concurrents peuvent laisser le contenu stocké

Le bug 003 décrit un write skew dans `SqlMailboxes.ack` : deux derniers destinataires qui acquittent simultanément peuvent chacun encore voir la livraison de l’autre dans son snapshot. Les deux livraisons disparaissent, mais aucune transaction ne vide l’enveloppe. Il faut sérialiser ce cas ou recalculer après suppression, avec un test concurrent.

## Correction

```workstream
commits
```

- 
