---
type: Review
title: "Le serveur ne garantit pas encore l’idempotence sous concurrence"
description: "Le serveur ne garantit pas encore l’idempotence sous concurrence"
tags: [review]
status: draft
state: todo
parent: stories/02-07-file-d-envoi-hors-ligne.md
review: rework
created: 2026-11-21
---

# Le serveur ne garantit pas encore l’idempotence sous concurrence

# Le serveur ne garantit pas encore l’idempotence sous concurrence

Le client persiste bien sa file et réessaie dans l’ordre. Mais le critère « le serveur ignore les doublons » dépend de 04-01 et échoue encore dans le cas concurrent décrit par le bug 002 : un même `SendId` peut recevoir un faux `staleEpoch` et consommer une séquence. La garantie « une seule fois » n’est donc pas complète.

## Correction

```workstream
commits
```

- 
