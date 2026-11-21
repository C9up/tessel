---
type: Journal
title: "Story 05-02 : conception des groupes"
status: draft
created: 2026-11-21
---

# Story 05-02 : conception des groupes

## Où vivent le nom et les rôles
Dans le contexte de groupe MLS, sous une extension propre à Tessel (type `0xF7A0`, plage privée) : nom, comptes admins, et drapeau « réservé aux admins ». C'est chiffré par construction : les commits sont des `PrivateMessage` et les Welcome chiffrent le GroupInfo. Le serveur n'en voit rien. Tous les membres en ont la même version à chaque epoch, et un commit (proposition `GroupContextExtensions`) la change. Une extension `RequiredCapabilities` l'impose : un appareil qui ne la connaît pas ne peut pas entrer.

## Qui peut quoi
Le contrôle a lieu à deux moments : quand on prépare un commit, et quand on traite un commit ou une proposition reçus. La même fonction est appelée des deux côtés, donc un client honnête refuse ce qu'un client modifié tenterait.
- Le créateur est admin.
- Admins **effectifs** : les comptes listés qui ont encore un appareil dans le groupe. Un groupe « réservé » sans admin effectif redevient ouvert (pas de groupe verrouillé à vie).
- Groupe réservé : seul un admin ajoute ou retire un **autre** compte. Chacun peut ajouter ou retirer ses propres appareils (liaison 03-04, révocation 03-05) et partir.
- Renommer : tout membre si le groupe est ouvert, un admin sinon. Changer les admins ou le drapeau : un admin, ou n'importe qui s'il n'y a plus d'admin effectif.
- L'auteur de chaque proposition compte, pas celui qui commite : un membre ordinaire peut donc commiter le départ d'un autre.

## Quitter
Un compte quitte avec tous ses appareils : un commit retire ses autres appareils (permis : c'est son compte), puis une proposition de départ pour celui-ci. Le premier membre qui la reçoit la commite.

## Limite connue
Un client modifié qui ferait accepter par le serveur un commit interdit bloquerait le groupe pour les autres, qui le refusent. Un membre malveillant peut déjà bloquer un groupe en envoyant un commit illisible ; la reprise (ré-initialisation du groupe) sort de cette story.
