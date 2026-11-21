---
type: Journal
title: "Story 04-07 bloquée : WebSocket"
status: draft
created: 2026-11-21
---

# Story 04-07 bloquée : WebSocket

La 04-07 (passer au WebSocket, SSE en repli) dépend de deux choses qui ne sont pas là :
- la story `relay` 01-05 (« Transport WebSocket pour relay ») est en état `todo`. Le README de relay le confirme : le serveur HTTP Rust de Ream n'expose pas de point de *upgrade* ;
- la mesure avant/après (latence, batterie) se fait sur Android, et la liaison mobile (02-13) est bloquée par atom 03-01.

Ce qui est déjà prêt pour le jour où relay saura faire du WebSocket : `listen()` (`@tessel/protocol/realtime`) ne dépend que d'une suite d'évènements et d'un abonnement. Seul le transport changera, et l'`Inbox` (curseur, accusés, sans perte ni doublon) restera la même.

Epic 04 : 04-01 à 04-06 en review, 04-07 bloquée.
