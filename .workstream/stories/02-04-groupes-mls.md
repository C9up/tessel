---
type: Story
title: "Groupes MLS"
description: "Créer un groupe, ajouter et retirer des membres et des appareils, traiter les commits."
tags: [product]
status: draft
state: todo
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-03-keypackages.md]
priority: high
created: 2026-11-20
---

# Groupes MLS

Créer un groupe, ajouter et retirer des membres et des appareils, traiter les commits.

## Critères d'acceptation

- Créer, ajouter (membres et appareils), retirer, quitter ; traiter *Commit* et *Welcome*.
- Un *Welcome* ou un groupe d'une autre suite que 0x0001 est refusé (ADR-001).
- Un commit refusé par le serveur (epoch dépassée) est rejoué sur l'état à jour, sans perte.
- Test à 3 comptes × 2 appareils : chacun lit tous les messages après ajouts et retraits ; un appareil retiré ne lit plus rien.

## Reviews

```workstream
reviews
```
