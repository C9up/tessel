---
type: Story
title: "Étude : le modèle des bots"
description: "Fixer ce qu'est un bot dans Tessel, ce qu'il voit et qui en répond, avant d'en écrire un."
tags: [product]
status: draft
state: todo
parent: epics/22-bots-et-integrations.md
depends_on: [stories/10-01-etude-espaces-salons-et-groupes-mls.md]
priority: backlog
created: 2026-11-20
---

# Étude : le modèle des bots

Fixer ce qu'est un bot dans Tessel, ce qu'il voit et qui en répond, avant d'en écrire un.

## Critères d'acceptation

- Comparaison relevée : bots Telegram, Discord et Matrix (appservices), comptes pilotés de Signal (`signal-cli`), et ce que chacun fait lire au serveur.
- Décision consignée (ADR) : un bot est un membre MLS ; comment le compte bot est marqué dans son certificat (02-02) pour que tout membre le vérifie ; ce que le serveur sait d'un bot, inscrit au registre des métadonnées (15-02).
- Bots de l'opérateur : comment ils sont marqués et qui peut les ajouter.

## Reviews

```workstream
reviews
```
