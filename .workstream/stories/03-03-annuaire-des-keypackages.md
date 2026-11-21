---
type: Story
title: "Annuaire des KeyPackages"
description: "Le serveur garde les KeyPackages de chaque appareil et en remet un, une seule fois, à qui veut l'inviter."
tags: [product]
status: draft
state: done
parent: epics/03-comptes-et-appareils.md
depends_on: [stories/03-02-authentification-par-la-cle-d-appareil.md, stories/02-03-keypackages.md]
priority: high
created: 2026-11-20
commits: [dd8c7af297b5336df3bfe9fc4573e66f62bc1fff]
---

# Annuaire des KeyPackages

Le serveur garde les KeyPackages de chaque appareil et en remet un, une seule fois, à qui veut l'inviter.

## Critères d'acceptation

- Dépôt par lots ; retrait atomique d'un *KeyPackage* par appareil (jamais remis deux fois).
- Le *KeyPackage* de dernier recours sert quand le stock est vide.
- L'appareil est prévenu quand son stock baisse.

## Reviews

```workstream
reviews
```
