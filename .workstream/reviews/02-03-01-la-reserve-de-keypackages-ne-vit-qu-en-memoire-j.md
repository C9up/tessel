---
type: Review
title: "La réserve de KeyPackages ne vit qu'en mémoire jusqu'à la 02-06"
description: "La réserve de KeyPackages ne vit qu'en mémoire jusqu'à la 02-06"
tags: [review]
status: draft
state: todo
parent: stories/02-03-keypackages.md
review: suggestion
created: 2026-11-20
---

# La réserve de KeyPackages ne vit qu'en mémoire jusqu'à la 02-06

La réserve de KeyPackages ne vit qu'en mémoire jusqu'à la 02-06

`Client::refresh_key_packages(now)` couvre les trois critères :
- par lots : une réserve de 100 *KeyPackages*, plus un de dernier recours ;
- remplacement de ce qui est consommé, ou retiré à moins de 2 semaines de la fin ;
- secret effacé seulement après la date de fin, car un *Welcome* peut encore être en route.

Ils sont sérialisés en `MLSMessage`, et `verify_key_package` refuse toute autre suite que 0x0001.

En revanche, la liste des *KeyPackages* publiés et leurs secrets sont dans le stockage mémoire d'OpenMLS. La base locale chiffrée (02-06) les rendra durables.

Les choix à valider : une réserve de 100 (comme les *prekeys* de Signal), une validité de 12 semaines (le défaut d'OpenMLS) et un renouvellement 2 semaines avant la fin.

## Correction

```workstream
commits
```

- 
