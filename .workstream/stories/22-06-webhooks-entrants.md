---
type: Story
title: "Webhooks entrants"
description: "Un bot standard qui poste dans un salon ce qu'un service externe lui envoie (alerte, CI, formulaire)."
tags: [product]
status: draft
state: todo
parent: epics/22-bots-et-integrations.md
depends_on: [stories/22-05-sdk-de-bots-sur-node.md]
priority: backlog
created: 2026-11-20
---

# Webhooks entrants

Un bot standard qui poste dans un salon ce qu'un service externe lui envoie (alerte, CI, formulaire).

## Critères d'acceptation

- Le webhook est reçu par le bot, pas par le serveur Tessel : le serveur ne voit que le message chiffré.
- Une URL secrète par salon, révocable ; un appel sans le bon secret est refusé.
- Hébergeable par son auteur ou, marqué comme tel, par l'opérateur de l'instance.

## Reviews

```workstream
reviews
```
