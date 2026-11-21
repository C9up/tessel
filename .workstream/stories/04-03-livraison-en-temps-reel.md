---
type: Story
title: "Livraison en temps réel"
description: "Les enveloppes arrivent en direct par SSE et partent par POST."
tags: [product]
status: draft
state: done
parent: epics/04-acheminement-des-messages.md
depends_on: [stories/04-01-boites-aux-lettres-des-appareils.md, stories/01-05-contrat-client-serveur-avec-comet.md]
priority: high
created: 2026-11-20
commits: [82b1cd5e1ed3c547a9773af7210538b6ee1a5737]
---

# Livraison en temps réel

Les enveloppes arrivent en direct par SSE et partent par POST.

## Critères d'acceptation

- Un canal `relay` par appareil, autorisé par sa session.
- Envoi par une méthode `comet` ; réception poussée par SSE dès l'arrivée.
- Après une coupure, l'appareil reprend depuis son dernier curseur, sans perte ni doublon.

## Reviews

```workstream
reviews
```
