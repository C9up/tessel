---
type: Epic
title: "Bots et intégrations"
description: "Des bots membres des salons, visibles de tous, ajoutés par les admins, écrits avec un SDK sur le cœur Rust : sans jamais faire lire le serveur. Après la v1."
tags: [product]
status: draft
order: 22
created: 2026-11-20
---

# Bots et intégrations

Des bots membres des salons, visibles de tous, ajoutés par les admins, écrits avec un SDK sur le cœur Rust : sans jamais faire lire le serveur. Après la v1.

## Périmètre

- Un bot est un compte à part entière, avec ses appareils et ses clés : un membre du groupe MLS. MLS ne connaît pas de membre qui écrit sans lire, donc un bot lit tout ce que son salon lui montre.
- Type de compte « bot », signalé partout (badge, liste des bots de chaque salon) : personne n'est lu par un bot sans le savoir.
- Ajout par un admin seulement, salon par salon ; ce que le bot peut faire passe par les rôles (epic 10).
- Commandes `/…` déclarées par le bot, réponses à boutons : de nouveaux genres de message (format de la story 02-05).
- SDK : `tessel-core` dans Node par une liaison NAPI (ADR-003), un paquet TypeScript au-dessus. Le bot tourne chez son auteur, avec ses clés.
- Webhooks entrants : un bot standard qui poste dans un salon ce qu'un service externe lui envoie.
- Bots hébergés par l'opérateur d'une instance : permis, toujours marqués « bot de l'opérateur », ajoutés seulement par un admin du salon (choix du propriétaire, 20.11.2026).

Hors périmètre : bots qui tournent dans le serveur et lisent sans être membres (contraire à « le serveur ne lit rien »).

## Terminé quand

- Un bot écrit avec le SDK, lancé sur une autre machine, répond à une commande `/…` dans un salon chiffré.
- Chaque membre du salon voit qu'un bot est présent, qui l'a ajouté et qui l'héberge.
- Un webhook entrant poste dans un salon sans que le serveur lise le message.

## Dépendances

- 

## Stories

```workstream
stories
```
