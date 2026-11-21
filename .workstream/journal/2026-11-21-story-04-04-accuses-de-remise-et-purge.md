---
type: Journal
title: "Story 04-04 : accusés de remise et purge"
status: draft
created: 2026-11-21
---

# Story 04-04 : accusés de remise et purge

Commit 960ddcc. `pnpm check` vert : Rust 110 + 8 + 2, web 34, serveur 60 (3 nouveaux), protocole 18 (1 nouveau). Le serveur démarre aussi en mode web (`ream dev`) avec le planificateur et la tâche de purge.

## Ce qui est fait
- **Accusé de remise** (`delivery.ack {through}`) : l'appareil a traité toutes ses enveloppes jusqu'à son curseur ; ses livraisons partent.
- **Effacement quand tout le monde a reçu** : une enveloppe que plus personne n'attend perd aussitôt son corps et son en-tête (type, groupe, epoch). Ne restent que sa clé de déduplication (appareil expéditeur, `SendId`) et sa place, pour qu'un renvoi tardif reçoive une réponse sans être rangé ni remis une seconde fois (critère 04-01 : clé gardée le temps de la purge).
- **L'inbox accuse réception après chaque lot traité**, jamais avant (`@tessel/protocol/realtime`).
- **Purge** : toute enveloppe gardée plus longtemps que `DELIVERY_RETENTION_DAYS` (30 par défaut) part, avec ses livraisons, qu'elle ait été reçue ou non. La purge tourne chaque heure (minute 17) sur le planificateur de Ream, verrouillé dans KeyDB pour ne tourner que sur une instance. Le fournisseur du planificateur et son préchargement ne sont chargés que dans les processus web, ni en test ni dans une commande.
- **Tests** :
  - le contenu reste tant que Carol n'a pas reçu, puis disparaît ;
  - un renvoi est reconnu et rien n'est remis de nouveau ;
  - un appareil n'accuse que ses propres livraisons, et seulement jusqu'au point qu'il indique ;
  - la purge ne touche que ce qui est plus vieux que la rétention.

## Écart à la story : pas de tâche `bay`
La story demandait une tâche `bay`. Son README renvoie lui-même les tâches récurrentes au planificateur de Ream (`@Schedule`, verrou distribué) : un second planificateur dans la file serait une seconde horloge. La purge est une seule instruction SQL idempotente : passer par une tâche `bay` ajouterait une file et un *worker* sans rien apporter. **À trancher par le propriétaire**, comme `eclipse` en 04-02.

## Choix faits seul
- **La rétention est comptée en jours** (`created_on`, déjà au jour près), réglable par variable d'environnement. La durée est exprimée avec `chronos` (`Duration`) dans `config/delivery.ts`.
- **Accusé cumulatif (« jusqu'à »)** plutôt qu'une liste d'identifiants : c'est le curseur de l'inbox, et une seule requête suffit.

## Registre des métadonnées (story 15-02)
Après accusé, une enveloppe ne garde que `sender_device`, `send_id`, `seq` et `created_on`, jusqu'à la purge.
