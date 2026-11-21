---
type: Journal
title: "Story 05-02 : groupes livrés"
status: draft
created: 2026-11-21
---

# Story 05-02 : groupes livrés

Commit `005cd9e`. Conception : entrée « Story 05-02 : conception des groupes » ; décision : ADR-008.

## Les critères
1. **Créer, nommer (nom chiffré), inviter, retirer, quitter** :
   - côté messenger : `createGroup(name, accounts)`, `invite`, `remove`, `configure`, `leave` ;
   - le nom vit dans le contexte MLS chiffré : un test vérifie qu'il n'apparaît en clair ni dans le commit ni dans le Welcome ;
   - la liste des conversations porte le nom.
2. **Admins** : `restrict` réserve aux admins l'ajout ou le retrait d'un autre compte, et le renommage. La même règle est vérifiée à la préparation d'un commit et au traitement de ce qu'on reçoit. Un commit forgé par un client modifié est refusé par tous (test : `a_forbidden_commit_is_refused_by_every_member`).
3. **Taille maximale documentée** : 1 000 appareils (ADR-008, mesures à l'appui). Les invitations partent par lots de 100 appareils.

## Tests
- Cœur : 7 tests de groupes (nom, nom invalide, restriction, ses propres appareils, commit forgé, départ et groupe sans admin, renommage rejoué après rejet).
- Contrat commun : un scénario qui passe sur le web (en direct et par messages) et sur la liaison mobile.
- Messenger : 7 tests (création nommée, lots, rejeu, départ commité, sortie, abandon sans rattrapage depuis la boîte, nouvelle proposition).
- Bout en bout sur le serveur réel : groupe « Famille » réservé ; Carol ne peut pas inviter, Alice si ; Carol retirée ne lit plus rien ; Bob part, son départ est commité par un autre membre.

## Points notables
- **Interblocage évité.** Commiter un départ depuis `receive()` puis rattraper (`drain()` dans `drain()`) bloquerait. Depuis la boîte, un commit qui perd l'epoch est abandonné, car le gagnant arrive juste après. Si ce gagnant a omis le départ, la proposition a expiré avec l'epoch : l'appareil qui part la renvoie jusqu'à son retrait. Ce suivi des départs en cours est en mémoire seulement ; après un redémarrage, l'appli devra relancer `leave`.
- Les aides de test de bout en bout (`messengerApp.ts`) sont partagées entre la 05-01 et la 05-02.
- Le cas d'un client modifié qui ferait accepter un commit interdit (groupe bloqué) est consigné dans l'ADR-008 ; la reprise sera une story à part.
