---
type: Review
title: "Le ping n’est pas testé depuis les deux hôtes clients"
description: "Le ping n’est pas testé depuis les deux hôtes clients"
tags: [review]
status: draft
state: todo
parent: stories/01-05-contrat-client-serveur-avec-comet.md
review: rework
created: 2026-11-21
---

# Le ping n’est pas testé depuis les deux hôtes clients

# Le ping n’est pas testé depuis les deux hôtes clients

Le contrat `@tessel/protocol`, le routeur `comet` et le test serveur du client partagé existent. En revanche, il n’y a pas encore de client web ni de liaison mobile exécutant `system.ping`; le test end-to-end ne couvre qu’un transport de test Node. Le critère « depuis les deux clients » reste à réaliser, ou doit être reformulé si la preuve d’isomorphisme est volontairement reportée aux stories 12-01 et 13-10.

## Correction

```workstream
commits
```

- 
