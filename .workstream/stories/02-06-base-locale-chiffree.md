---
type: Story
title: "Base locale chiffrée"
description: "L'état MLS et l'historique sont chiffrés sur l'appareil, sur le web comme sur Android."
tags: [product]
status: draft
state: todo
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-02-identite-de-compte-et-d-appareil.md]
priority: high
created: 2026-11-20
---

# Base locale chiffrée

L'état MLS et l'historique sont chiffrés sur l'appareil, sur le web comme sur Android.

## Critères d'acceptation

- Un contrat de stockage unique dans le cœur, deux implémentations : SQLite chiffré en natif, IndexedDB côté web.
- Web : clé WebCrypto non extractible ; Android : clé protégée par le Keystore.
- Migrations de schéma versionnées et testées.

## Reviews

```workstream
reviews
```
