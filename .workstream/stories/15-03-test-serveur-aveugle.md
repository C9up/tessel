---
type: Story
title: "Test « serveur aveugle »"
description: "La preuve automatique que le serveur ne contient aucun message lisible."
tags: [product]
status: draft
state: todo
parent: epics/15-securite-et-audit.md
depends_on: [stories/05-02-groupes.md, stories/06-02-photos.md, stories/08-01-transferer-l-historique-vers-un-nouvel-appareil.md, stories/07-02-web-push.md]
priority: high
created: 2026-11-20
---

# Test « serveur aveugle »

La preuve automatique que le serveur ne contient aucun message lisible.

## Critères d'acceptation

- Le scénario de la première étape réussie est joué de bout en bout (deux comptes, groupe, photos, second appareil, push).
- La base, le stockage S3 et les journaux sont fouillés : aucun texte, nom de groupe, nom de fichier ni pixel de photo en clair.
- Le test tourne dans `pnpm check`.

## Reviews

```workstream
reviews
```
