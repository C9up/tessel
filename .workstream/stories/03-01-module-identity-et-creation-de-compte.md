---
type: Story
title: "Module identity et création de compte"
description: "Créer un compte avec un simple pseudo, sans e-mail ni numéro."
tags: [product]
status: draft
state: review
parent: epics/03-comptes-et-appareils.md
depends_on: [stories/01-02-serveur-ream-sur-le-modele-nadeo.md, stories/02-02-identite-de-compte-et-d-appareil.md]
priority: high
created: 2026-11-20
---

# Module identity et création de compte

Créer un compte avec un simple pseudo, sans e-mail ni numéro.

## Critères d'acceptation

- Pseudo unique sur le serveur ; identité `pseudo@domaine`.
- Validation par `rune` ; limitation des créations par `blackhole`.
- Aucune donnée personnelle demandée.
- Événement `identity.account.created` publié sur le bus.

## Reviews

```workstream
reviews
```
