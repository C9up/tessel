---
type: Bug
title: "Deux accusés concurrents peuvent laisser le corps d’une enveloppe orpheline"
description: "Chaque requête peut supprimer sa livraison puis, dans son snapshot, encore voir la livraison supprimée concurremment par l’autre transaction. Aucune ne vide alors `body`, `kind`, `group_id` et `epoch`, alors qu’il ne reste plus de destinataire."
tags: [bug, product]
status: draft
state: todo
parent: epics/04-acheminement-des-messages.md
severity: major
created: 2026-11-21
---

# Deux accusés concurrents peuvent laisser le corps d’une enveloppe orpheline

## Reproduire

1. Créer une enveloppe destinée à deux appareils.
2. Faire appeler `delivery.ack` simultanément par les deux appareils, chacun jusqu’à cette enveloppe.
3. Après validation des deux transactions, lire les lignes `deliveries` et l’enveloppe.

## Constaté

Chaque requête peut supprimer sa livraison puis, dans son snapshot, encore voir la livraison supprimée concurremment par l’autre transaction. Aucune ne vide alors `body`, `kind`, `group_id` et `epoch`, alors qu’il ne reste plus de destinataire.

## Attendu

La transaction qui enlève la dernière livraison efface toujours le corps et l’en-tête de l’enveloppe, même si les derniers accusés sont simultanés.

## Environnement

PostgreSQL, `SqlMailboxes.ack`, branche main au 2026-11-21

## Correction

```workstream
commits
```

- 

## Reviews

```workstream
reviews
```
