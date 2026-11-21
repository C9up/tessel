---
type: Review
title: "La révocation n’orchestre pas encore le retrait de tous les groupes"
description: "La révocation n’orchestre pas encore le retrait de tous les groupes"
tags: [review]
status: draft
state: todo
parent: stories/03-05-liste-et-revocation-des-appareils.md
review: rework
created: 2026-11-21
---

# La révocation n’orchestre pas encore le retrait de tous les groupes

# La révocation n’orchestre pas encore le retrait de tous les groupes

Le serveur coupe bien sessions et KeyPackages, et le cœur sait produire les commits de retrait. Mais l’hôte ne parcourt pas encore `groups_with(device)` pour envoyer ces commits par le transport réel, comme le journal le signale. Les contacts ne reçoivent donc pas encore l’avertissement dans un parcours réel. Les critères de retrait de tous les groupes et d’avertissement restent incomplets.

## Correction

```workstream
commits
```

- 
