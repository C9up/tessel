---
type: Story
title: "Messages applicatifs"
description: "Le format interne des messages (texte, médias, contrôle), chiffré dans MLS."
tags: [product]
status: draft
state: todo
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-04-groupes-mls.md]
priority: high
created: 2026-11-20
---

# Messages applicatifs

Le format interne des messages (texte, médias, contrôle), chiffré dans MLS.

## Critères d'acceptation

- Format versionné : texte, référence de média, accusé, réponse, édition, suppression.
- Chiffrer et déchiffrer via le groupe MLS ; tout message mal formé est rejeté sans planter.
- Remplissage (*padding*) pour que la taille chiffrée ne trahisse pas la longueur du texte.

## Reviews

```workstream
reviews
```
