---
type: Journal
title: "Story 03-07 : supprimer son compte"
status: draft
created: 2026-11-21
---

# Story 03-07 : supprimer son compte

Commit 3ee4210. `pnpm check` vert : Rust 107 + 8 + 2 (2 nouveaux), web 34, serveur 47 (3 nouveaux), protocole 11.

## Ce qui est fait
- **Signature par la clé de compte**, comme pour révoquer un autre appareil (03-05) : label « Tessel 1.0 AccountDeletion » sur (version, compte). Une tablette volée ne peut pas supprimer le compte. Rejouer une suppression ne sert à rien : le nom libéré ira à une autre clé, sur laquelle l'ancienne signature ne vérifie pas.
- **Quitter tous ses groupes d'abord** : l'appareil appelle `leave_group` dans chaque groupe avant d'envoyer la suppression. Le test Rust montre Alice qui quitte, Bob qui valide son départ, et Alice qui reçoit `removed_self`.
- **Effacement côté serveur** (`identity.deleteAccount`), en une seule requête : le compte, ses appareils, sessions, KeyPackages et profil partent par cascade, et le nom entre dans `reserved_names` pour 30 jours. Le test compte les lignes avant (1 compte, 2 appareils, 2 sessions, 101 KeyPackages, 1 profil) et après (0 partout, 1 réservation).
- **Le pseudo reste réservé, puis se libère** : la création d'un compte refuse un nom réservé (`E_IDENTITY_NAME_TAKEN`). Les réservations échues sont effacées à chaque création et à chaque suppression : aucune tâche planifiée n'est nécessaire.
- **Événement `identity.account.deleted`** : les modules à venir qui garderont quelque chose d'un compte (sauvegardes en epic 07, médias en attente en epic 05) devront l'écouter et effacer leur part. **À reporter dans leurs stories** : ce critère de la 03-07 ne pourra être vérifié qu'à ce moment-là.

## Choix faits seul
- **Réservation de 30 jours**, assez pour que les contacts voient le compte disparaître avant qu'un autre ne prenne le nom. La réservation garde le nom en clair, ce que le serveur connaissait déjà, et rien d'autre.
- **Les autres appareils du compte**, comme la tablette, perdent leurs sessions mais ne quittent pas eux-mêmes leurs groupes. Les autres membres les retireront à réception de `identity.account.deleted`, relayé par l'epic 04 (`Removal::Account`).
- **L'effacement des données locales de l'appareil** (IndexedDB) n'est pas dans cette story, qui parle du serveur. Il reviendra au client web (epic 12).

## Registre des métadonnées (story 15-02)
- `reserved_names.name` et `reserved_until` : le nom d'un compte supprimé, pendant 30 jours.
- Événement `identity.account.deleted` : identifiant opaque du compte, heure.
- Compteur `identity_delete_<appareil>` : une heure.

## Epic 03
Les sept stories sont en review. Ce qui reste hors de l'epic :
- l'acheminement des commits, *Welcome* et messages de profil entre appareils réels (epic 04) ;
- le pont Android (02-13, bloqué par Ream) ;
- l'interface (epic 12).
