---
type: Story
title: "Profil chiffré"
description: "Nom affiché et avatar partagés avec ses contacts, sans que le serveur les voie."
tags: [product]
status: draft
state: doing
parent: epics/03-comptes-et-appareils.md
depends_on: [stories/03-01-module-identity-et-creation-de-compte.md, stories/02-08-chiffrement-des-medias.md]
priority: medium
created: 2026-11-20
commits: [29aa5a0e3b3082922a7d190f8ee0e66c8ba0eea8]
---

# Profil chiffré

Nom affiché et avatar partagés avec ses contacts, sans que le serveur les voie.

## Critères d'acceptation

- Nom et avatar chiffrés par une clé de profil, transmise aux contacts par MLS.
- Le serveur ne stocke que le blob chiffré du profil.
- Changer de profil prévient les contacts.

## Reviews

```workstream
reviews
```
