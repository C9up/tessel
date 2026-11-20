---
type: Journal
title: "Découpage en epics et stories"
status: draft
created: 2026-11-20
---

# Découpage en epics et stories

Demande du propriétaire (20.11.2026) : « fait tous les epic / stories ».

**Découpage.** 21 epics, 113 stories. La priorité dit à quel moment une story compte :
- `high` (50) : la première étape réussie de la vision (web + Android, texte et photos chiffrés, petit groupe, second appareil avec historique, push sans contenu, codes de sécurité, test « serveur aveugle »).
- `medium` / `low` (40) : le reste de la v1 (espaces, rôles, contacts et numéro, sauvegarde par phrase, modération, vocaux et fichiers, exploitation).
- `backlog` (23) : après la v1, epics 16 à 21 (appels, suite Discord, vie privée renforcée, iOS / UnifiedPush / bureau, fédération, passerelles), plus 04-07 (WebSocket) et 15-05 (audit externe).

Organisation : les epics 02 à 11 portent le cœur et les modules serveur ; les epics 12 (web) et 13 (Android) portent les écrans, qui dépendent des stories métier. Fichiers générés par script au format exact de `create_epic` / `create_story` (pour éviter une course aux numéros), dépendances vérifiées.

**Travail pour Ream, dans les Workstream des paquets** (chaque paquet a son `.workstream`) :
- WebSocket : la story existe déjà, `relay` 01-05 « Transport WebSocket pour relay » (backlog, upgrade dans `ream-http`). Constat vérifié : `ream-http` sert ses connexions par `serve_connection` d'hyper 1, sans `with_upgrades`. Pas de doublon ; Tessel y renvoie (04-07, epic 16). À remonter en priorité avant les appels.
- Push natif : epic `nova` 03 « Native push: FCM, APNs, UnifiedPush » créée (5 stories). **Point d'attention** : l'ADR-002 de Ream et la vision de nova limitent nova au Web Push, et disent que tout élargissement passe par un nouvel ADR. La story nova 03-01 est donc cet ADR, à valider par le mainteneur avant tout code. Le choix « étendre nova » du cadrage (16.11) avait été recommandé sans connaître l'ADR-002 ; l'alternative (nouveau paquet) reste ouverte dans l'ADR.
