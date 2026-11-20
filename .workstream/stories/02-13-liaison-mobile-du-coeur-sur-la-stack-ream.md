---
type: Story
title: "Liaison mobile du cœur sur la stack Ream"
description: "Le cœur atteint le téléphone par la liaison que la stack mobile de Ream définit (ABI C + JSI), avec la même API que sur le web."
tags: [product]
status: draft
state: todo
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-07-file-d-envoi-hors-ligne.md, stories/02-08-chiffrement-des-medias.md, stories/02-09-codes-de-securite.md]
priority: high
created: 2026-11-20
---

# Liaison mobile du cœur sur la stack Ream

Le cœur atteint le téléphone par la liaison que la stack mobile de Ream définit (ABI C + JSI), avec la même API que sur le web.

## Critères d'acceptation

- La liaison suit le modèle de la stack mobile de Ream (`ream:epics/73-mobile-apps-on-the-ream-ecosystem-without-react.md`) et de la liaison mobile d'atom (`atom:epics/03-a-mobile-binding-for-the-rust-engine.md`) ; ce qui lui manque devient du travail pour Ream, pas un contournement dans Tessel.
- Un crate de liaison mince (ABI C) au-dessus de `tessel-core`, construit pour `aarch64-linux-android` et `x86_64-linux-android`.
- Même API TypeScript que `@tessel/core-web` (interface `TesselCore`), vérifiée par un test de contrat commun.
- Aucune clé privée ne traverse la liaison.

## Reviews

```workstream
reviews
```
