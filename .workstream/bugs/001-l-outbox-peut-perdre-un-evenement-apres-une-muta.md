---
type: Bug
title: "L’outbox peut perdre un événement après une mutation métier"
description: "La mutation métier est déjà validée. `enqueueForDurability` écrit ensuite avec une opération séparée, intercepte toute erreur et poursuit sur le fast path. Après une interruption ou une panne de base au mauvais moment, aucune ligne d’outbox ne permet de rejouer l’événement."
tags: [bug, product]
status: draft
state: todo
parent: epics/01-fondations-du-depot.md
severity: major
created: 2026-11-21
---

# L’outbox peut perdre un événement après une mutation métier

## Reproduire

1. Exécuter une mutation métier qui écrit en base, par exemple la création d’un compte.
2. Faire échouer l’insertion dans `outbox`, ou interrompre le processus entre la mutation et `DomainEvent.emit()`.
3. Relancer le relais d’outbox.

## Constaté

La mutation métier est déjà validée. `enqueueForDurability` écrit ensuite avec une opération séparée, intercepte toute erreur et poursuit sur le fast path. Après une interruption ou une panne de base au mauvais moment, aucune ligne d’outbox ne permet de rejouer l’événement.

## Attendu

La mutation métier et sa ligne d’outbox sont validées dans la même transaction ; si l’outbox ne peut pas être écrite, la mutation ne doit pas être validée.

## Environnement

Serveur Ream, module shared/events, branche main au 2026-11-21

## Correction

```workstream
commits
```

- 

## Reviews

```workstream
reviews
```
