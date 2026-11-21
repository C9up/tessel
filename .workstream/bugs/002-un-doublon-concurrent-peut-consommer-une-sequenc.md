---
type: Bug
title: "Un doublon concurrent peut consommer une séquence de groupe et être refusé"
description: "Les deux CTE `existing` peuvent être vides. La seconde requête peut faire avancer `next_seq` dans `placed`, puis perdre l’`INSERT ... ON CONFLICT` sur l’enveloppe. Son snapshot ne voit pas l’enveloppe gagnante : elle ne rend aucun identifiant et est traduite en `E_DELIVERY_STALE_EPOCH`, avec un trou possible dans les séquences."
tags: [bug, product]
status: draft
state: todo
parent: epics/04-acheminement-des-messages.md
severity: major
created: 2026-11-21
---

# Un doublon concurrent peut consommer une séquence de groupe et être refusé

## Reproduire

1. Depuis le même appareil, envoyer simultanément deux requêtes `delivery.send` identiques avec le même `SendId`, le même groupe et la même epoch.
2. Laisser les deux requêtes entrer dans `SqlMailboxes.#storeInGroup` avant que la première ne valide.
3. Observer les réponses et `delivery_groups.next_seq`.

## Constaté

Les deux CTE `existing` peuvent être vides. La seconde requête peut faire avancer `next_seq` dans `placed`, puis perdre l’`INSERT ... ON CONFLICT` sur l’enveloppe. Son snapshot ne voit pas l’enveloppe gagnante : elle ne rend aucun identifiant et est traduite en `E_DELIVERY_STALE_EPOCH`, avec un trou possible dans les séquences.

## Attendu

Toutes les requêtes concurrentes du même couple (appareil expéditeur, `SendId`) rendent la même enveloppe et la même séquence, sans faire avancer deux fois l’ordre du groupe.

## Environnement

PostgreSQL, `SqlMailboxes.#storeInGroup`, branche main au 2026-11-21

## Correction

```workstream
commits
```

- 

## Reviews

```workstream
reviews
```
