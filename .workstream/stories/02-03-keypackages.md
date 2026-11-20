---
type: Story
title: "KeyPackages"
description: "Le cœur fabrique et renouvelle les KeyPackages que le serveur distribue pour qu'on puisse inviter un appareil."
tags: [product]
status: draft
state: review
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-02-identite-de-compte-et-d-appareil.md]
priority: high
created: 2026-11-20
---

# KeyPackages

Le cœur fabrique et renouvelle les KeyPackages que le serveur distribue pour qu'on puisse inviter un appareil.

## Critères d'acceptation

- Génération par lots, plus un *KeyPackage* de dernier recours.
- Rotation : les KeyPackages consommés ou trop vieux sont remplacés.
- Format sérialisé stable, prêt à être déposé sur le serveur.
- Un *KeyPackage* d'une autre suite que 0x0001 est refusé (ADR-001).

## Reviews

```workstream
reviews
```
