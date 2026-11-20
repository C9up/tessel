---
type: Story
title: "Chiffrement des médias"
description: "Chaque fichier est chiffré sur l'appareil avec sa propre clé, transmise dans le message MLS."
tags: [product]
status: draft
state: todo
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-05-messages-applicatifs.md]
priority: high
created: 2026-11-20
---

# Chiffrement des médias

Chaque fichier est chiffré sur l'appareil avec sa propre clé, transmise dans le message MLS.

## Critères d'acceptation

- Clé aléatoire par fichier ; chiffrement authentifié par blocs, en flux, pour les gros fichiers.
- La clé et l'empreinte voyagent dans le message MLS, jamais séparément.
- Un blob modifié est détecté et refusé au déchiffrement.

## Reviews

```workstream
reviews
```
