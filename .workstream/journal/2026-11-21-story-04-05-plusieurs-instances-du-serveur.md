---
type: Journal
title: "Story 04-05 : plusieurs instances du serveur"
status: draft
created: 2026-11-21
---

# Story 04-05 : plusieurs instances du serveur

Commit 7a916de. `pnpm check` vert : Rust 110 + 8 + 2, web 34, serveur 61 (1 nouveau), protocole 18.

## Ce qui est fait
- **Diffusion entre instances par le bus de `relay`** : `transports.redis({ connection: 'main' })` (quasar, KeyDB), canal `tessel::relay`. Une sonnette émise par une instance atteint les flux ouverts sur les autres.
- **Rien d'autre n'avait à changer** :
  - les enveloppes sont dans PostgreSQL, que toutes les instances partagent ;
  - l'ordre par groupe tient au verrou de ligne (04-02), valable sur toutes les instances ;
  - la purge tourne sur une seule instance grâce au verrou KeyDB du planificateur (04-04) ;
  - les limites de débit `blackhole` comptent en mémoire par instance (à revoir en 04-06).
- **Test à deux instances** :
  - `tests/instance.ts` démarre une seconde instance dans son propre processus, parce que le service de `relay` est unique dans un processus ;
  - Bob écoute et récupère ses messages sur la seconde instance, Alice envoie par la première ;
  - Bob reçoit aussitôt son message, par la sonnette passée par KeyDB ;
  - le processus est arrêté à la fin du test (`SIGTERM`).

## Choix faits seul
- **Le bus ne transporte que des sonnettes** (identifiant d'enveloppe), jamais un corps : c'est la raison de la conception choisie en 04-03.
- **Les flux restent attachés à une instance** : un appareil s'abonne sur l'instance qui tient son flux. Un abonnement arrivé sur une autre instance est relayé par `relay` lui-même. Il n'y a pas de *hubs* SignalR dans Tessel, donc pas besoin de sessions collantes.

## Registre des métadonnées (story 15-02)
Sur KeyDB, en pub/sub et sans rien garder : le canal de l'appareil (`devices/<id>`) et l'identifiant de l'enveloppe, au moment où elle arrive.
