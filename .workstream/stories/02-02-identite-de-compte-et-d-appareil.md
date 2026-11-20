---
type: Story
title: "Identité de compte et d'appareil"
description: "Chaque appareil a sa clé, liée au compte `pseudo@domaine` par une signature vérifiable."
tags: [product]
status: draft
state: review
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-01-integrer-openmls-et-la-suite-cryptographique.md]
priority: high
created: 2026-11-20
---

# Identité de compte et d'appareil

Chaque appareil a sa clé, liée au compte `pseudo@domaine` par une signature vérifiable.

## Critères d'acceptation

- Une clé d'identité de compte et une clé de signature par appareil.
- Le *credential* MLS d'un appareil prouve qu'il appartient au compte (signature croisée vérifiable par tout membre).
- Les clés privées restent dans la base locale chiffrée ; aucune fonction de l'API ne les rend.

## Reviews

```workstream
reviews
```
