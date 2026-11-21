---
type: Journal
title: "Story 04-03 : livraison en temps réel"
status: draft
created: 2026-11-21
---

# Story 04-03 : livraison en temps réel

Commit 82b1cd5. `pnpm check` vert : Rust 110 + 8 + 2, web 34, serveur 57 (2 nouveaux, stables sur 3 passages), protocole 17 (4 nouveaux).

## Ce qui est fait
- **Un canal `relay` par appareil, autorisé par sa session** :
  - un middleware (`streamGate`) placé sur les routes de `relay` vérifie le jeton de session (`Authorization: Bearer`) et fait de l'appareil l'utilisateur de la requête (`ctx.auth`). `relay` s'en sert pour savoir à qui appartient un flux ;
  - le canal `devices/:id` n'est autorisé qu'à l'appareil lui-même : un autre reçoit 403 ;
  - sans session : 401.
- **Envoi par une méthode `comet`** (`delivery.send`, déjà là) ; **réception poussée** : dès qu'une enveloppe est rangée, le serveur sonne le canal de chaque destinataire.
- **La sonnette ne porte que l'identifiant de l'enveloppe**, jamais son corps. L'appareil va chercher ce qui est arrivé depuis son curseur (`delivery.fetch`) : la boîte reste la seule source de vérité.
- **Reprise sans perte ni doublon** (`@tessel/protocol/realtime`, partagé par le web et le mobile) :
  - `Inbox` livre chaque enveloppe une seule fois, dans l'ordre, et n'avance son curseur qu'une fois l'enveloppe traitée ;
  - une sonnette qui arrive pendant une récupération en relance une ;
  - `listen()` ouvre le flux avec `fetch` (contrairement à `EventSource`, il peut porter `Authorization`), s'abonne, rattrape depuis le curseur, puis suit les sonnettes ;
  - `readSse` lit les trames.
- **Test de bout en bout** : Bob écoute. Le message d'Alice lui arrive aussitôt. Le flux est coupé, Alice écrit deux fois, Bob revient et reçoit les deux, puis le suivant en direct, rien en double.

## Choix faits seul
- **Une sonnette plutôt que le message dans le flux**. Raisons :
  - rien ne se perd si une sonnette se perd ;
  - l'ordre est celui du serveur, alors que deux diffusions concurrentes peuvent arriver dans le désordre ;
  - avec plusieurs instances (04-05), aucun corps ne passe par le bus KeyDB.

  Le coût : un aller-retour de plus par message.
- **Les routes d'abonnement de `relay` sont exemptées de CSRF**, comme `/rpc` : aucun cookie n'est en jeu.
- **Une trame de maintien toutes les 25 s**, pour que les proxys ne coupent pas un flux silencieux ; au plus 4 canaux par flux.

## Registre des métadonnées (story 15-02)
Rien de nouveau en base. En mémoire, pendant la connexion : quel appareil a un flux ouvert. Les compteurs de `relay` (flux par appareil) ne sortent pas du processus.
