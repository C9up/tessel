---
type: Review
title: "La suppression complète du compte n’est pas encore orchestrée"
description: "La suppression complète du compte n’est pas encore orchestrée"
tags: [review]
status: draft
state: todo
parent: stories/03-07-supprimer-son-compte.md
review: rework
created: 2026-11-21
---

# La suppression complète du compte n’est pas encore orchestrée

# La suppression complète du compte n’est pas encore orchestrée

Le serveur efface correctement les données du module identity et réserve le pseudo. En revanche, quitter tous les groupes avant suppression reste une convention d’appel non orchestrée ; les autres appareils du compte ne sont pas retirés. Les sauvegardes et médias ne sont pas effacés : leurs futurs modules devront écouter `identity.account.deleted`, événement qui peut en outre être perdu tant que le bug 001 n’est pas corrigé. Le premier critère n’est donc pas satisfait de bout en bout.

## Correction

```workstream
commits
```

- 
