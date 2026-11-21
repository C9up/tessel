---
type: Décision
title: "ADR-008 — Réglages et taille des groupes"
description: "Le nom, les admins et la restriction d'un groupe vivent dans le contexte de groupe MLS (extension 0xF7A0) ; un groupe compte au plus 1 000 appareils."
tags: [adr, crypto, groupes]
status: draft
decision_status: "Proposée"
created: 2026-11-21
---

# ADR-008 — Réglages et taille des groupes

Story 05-02.

## Réglages : dans le contexte de groupe MLS

Le nom, la liste des comptes admins et le drapeau « réservé aux admins » sont encodés dans une extension de contexte de groupe propre à Tessel (`GROUP_SETTINGS_EXTENSION = 0xF7A0`, plage privée de MLS). Une extension `RequiredCapabilities` l'exige de tout membre, et les KeyPackages de Tessel la déclarent.

Pourquoi là :
- **Chiffré par construction** : un changement passe par un commit (`PrivateMessage`) ; un nouvel appareil l'apprend par le Welcome, qui chiffre le `GroupInfo`. Le serveur ne voit ni le nom ni les rôles.
- **Le même état chez tous** : le contexte fait partie de l'epoch, et un membre ne peut pas avoir un autre nom de groupe que les autres sans sortir du groupe.
- **Pas de canal de plus** : pas de message applicatif « renommage » à ordonner à part, l'ordre des commits du serveur (story 04-02) suffit.

Écarté : un message applicatif chiffré portant le nom. Il se perd avec l'historique d'un nouvel appareil, et deux renommages simultanés n'ont pas d'ordre défini.

## Règles

Elles sont vérifiées à la préparation d'un commit et au traitement d'un commit ou d'une proposition reçus, par la même fonction (`Rules`, `crates/tessel-core/src/groups/settings.rs`).
- Le créateur est admin. Seuls comptent les admins qui ont encore un appareil dans le groupe ; un groupe réservé sans admin redevient ouvert.
- Dans un groupe réservé, seul un admin ajoute ou retire un autre compte. Chacun gère ses propres appareils et peut partir.
- Renommer suit la même règle. Changer les admins ou la restriction : un admin, ou n'importe qui s'il n'en reste aucun.
- L'auteur de chaque proposition compte, pas celui qui commite.

## Taille : 1 000 appareils au plus

`MAX_GROUP_DEVICES = 1000`, refusé à la préparation, au traitement d'un commit et à l'entrée par Welcome. Avec 5 appareils par personne, cela fait environ 200 personnes. Tessel vise les groupes de proches, pas les communautés publiques.

Mesures (OpenMLS 0.9.2, suite 0x0001, mode release, 21.11.2026) :

| Groupe | Commit d'ajout d'un appareil | Welcome de cet appareil |
|---|---|---|
| 50 appareils | 5,3 Ko | 18,6 Ko |
| 200 appareils | 17,7 Ko | 70,1 Ko |
| 999 appareils | 83,2 Ko | 344,0 Ko |

Les plafonds du serveur (story 04-06) sont de 256 Ko pour un commit, 512 Ko pour un Welcome et 1 000 destinataires par enveloppe : à 1 000 appareils, tout passe avec de la marge. Le Welcome grossit d'environ 345 octets par appareil, car il transporte l'arbre entier.

Une invitation en masse ajoute environ 450 octets de KeyPackage par appareil au commit. `@tessel/messenger` invite donc par lots de 100 appareils au plus, ce qui donne au pire environ 130 Ko au plafond.

## Conséquences

- Un client modifié qui ferait accepter par le serveur un commit interdit est refusé par tous les autres, et le groupe reste bloqué à cette epoch. Un membre malveillant le peut déjà avec un commit illisible ; la reprise (ré-initialisation du groupe) sera une story à part.
- Changer le format de l'extension passera par sa version (octet de tête).
