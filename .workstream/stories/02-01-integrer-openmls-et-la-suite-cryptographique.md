---
type: Story
title: "Intégrer OpenMLS et la suite cryptographique"
description: "OpenMLS épinglé, la suite choisie, et le tout compile sur les trois cibles."
tags: [product]
status: draft
state: review
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/01-04-squelette-de-tessel-core-et-chaine-de-compilatio.md, stories/01-07-decisions-d-architecture-de-depart.md]
priority: high
created: 2026-11-20
---

# Intégrer OpenMLS et la suite cryptographique

OpenMLS épinglé, la suite choisie, et le tout compile sur les trois cibles.

## Critères d'acceptation

- Suite cryptographique consignée dans la décision de 01-07 (par exemple `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`).
- OpenMLS épinglé à une version publiée ; `cargo deny` accepte sa licence et ses dépendances.
- Compile en natif, en WASM et pour Android.

## Reviews

```workstream
reviews
```
