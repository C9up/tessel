---
type: Story
title: "File d'envoi hors ligne"
description: "Un message écrit sans réseau part tout seul au retour de la connexion, une seule fois, dans l'ordre."
tags: [product]
status: draft
state: review
parent: epics/02-tessel-core-le-coeur-cryptographique.md
depends_on: [stories/02-05-messages-applicatifs.md, stories/02-06-base-locale-chiffree.md]
priority: high
created: 2026-11-20
---

# File d'envoi hors ligne

Un message écrit sans réseau part tout seul au retour de la connexion, une seule fois, dans l'ordre.

## Critères d'acceptation

- Les messages en attente sont persistés dans la base locale.
- Renvoi automatique au retour du réseau ; identifiant client pour que le serveur ignore les doublons.
- L'ordre est conservé par conversation.

## Reviews

```workstream
reviews
```
