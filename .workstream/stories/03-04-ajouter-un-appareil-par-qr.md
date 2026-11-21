---
type: Story
title: "Ajouter un appareil par QR"
description: "Le téléphone et le web se lient en scannant un QR, sans mot de passe."
tags: [product]
status: draft
state: doing
parent: epics/03-comptes-et-appareils.md
depends_on: [stories/03-03-annuaire-des-keypackages.md, stories/02-04-groupes-mls.md]
priority: high
created: 2026-11-20
commits: [9e671af52917b94df99b7b97f32d23250d45e325]
---

# Ajouter un appareil par QR

Le téléphone et le web se lient en scannant un QR, sans mot de passe.

## Critères d'acceptation

- Un appareil connecté et le nouvel appareil s'échangent un secret par QR, puis un canal éphémère chiffré.
- Le nouvel appareil reçoit son *credential* signé et rejoint tous les groupes du compte.
- Fonctionne web → Android et Android → web.

## Reviews

```workstream
reviews
```
