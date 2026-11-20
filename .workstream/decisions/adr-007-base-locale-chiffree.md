---
type: Décision
title: "ADR-007 — Base locale chiffrée : le cœur scelle, l'hôte range des octets"
description: "Le cœur scelle chaque enregistrement (ChaCha20-Poly1305 d'OpenMLS, identifiants HMAC, remplissage Padmé) ; SQLite en natif et IndexedDB dans le navigateur ne rangent que des octets opaques. La clé de la base est tirée par le cœur et scellée par le magasin de clés de la plateforme."
tags: [adr, coeur, stockage]
status: draft
decision_status: "Proposée"
created: 2026-11-20
---

# ADR-007 — Base locale chiffrée : le cœur scelle, l'hôte range des octets

## Contexte

Story 02-06 : l'état MLS et l'historique doivent être chiffrés sur l'appareil, avec un contrat unique dans le cœur et deux implémentations (SQLite en natif, IndexedDB dans le navigateur).

Trois contraintes :
- **OpenMLS lit et écrit son état de façon synchrone**, alors qu'IndexedDB ne répond que de façon asynchrone.
- **Pas de cryptographie maison**, et toute manipulation de clé vit dans `tessel-core`.
- **Une édition ou une suppression ne s'applique que si son expéditeur a le même compte que l'auteur du message visé**. La vérification doit se faire pendant le traitement du message, donc de façon synchrone.

## Décision

### 1. Le cœur scelle chaque enregistrement

- **Chiffrement** : ChaCha20-Poly1305 du fournisseur d'OpenMLS, avec un nonce aléatoire de 12 octets.
- **Clés** : dérivées par HKDF-SHA256 de la clé de la base, une pour chiffrer, une pour les identifiants.
- **Partition et identifiant** : chacun fait 32 octets et vient d'un HMAC. L'hôte ne peut savoir ni ce qu'est un enregistrement, ni à quel groupe il appartient. Ils servent aussi de données associées : un enregistrement déplacé ou échangé sur le disque ne s'ouvre plus.
- **Remplissage** : le texte clair est complété jusqu'à son palier Padmé, comme les messages (02-05).
- **Versions** : chaque enregistrement commence par un octet de version (`STORE_FORMAT`). Un enregistrement d'une version plus récente est refusé en le nommant.

Aucune nouvelle dépendance cryptographique n'est ajoutée.

### 2. Les hôtes ne rangent que des octets

- **Natif** : `SqliteStore`, derrière la fonctionnalité `sqlite`.
  - SQLite est compilé dans le binaire et `secure_delete` est activé.
  - Migrations : une liste, avec `PRAGMA user_version`. Chaque migration tourne dans sa propre transaction, et une base plus récente est refusée.
  - La fonctionnalité `sqlite` est coupée par défaut, parce que compiler SQLite pour Android demande le NDK. La liaison mobile (02-13) l'activera.
- **Navigateur** : `IndexedDbStore` (`@tessel/core-web`).
  - Migrations : une liste, appliquée dans la transaction de mise à niveau.
  - Une base écrite par une version plus récente est refusée.

**Pas de SQLCipher**, pour trois raisons :
- il n'a pas d'équivalent pour IndexedDB ;
- les deux hôtes auraient deux méthodes de chiffrement différentes ;
- il ajouterait une bibliothèque cryptographique en C.

### 3. Le contrat

- **En mémoire** : le cœur garde son état de travail en mémoire.
- **Après chaque appel qui le modifie** : l'hôte appelle `take_changes()`. Il applique les changements en une seule transaction, **avant d'agir sur la sortie de l'appel** (envoyer, accuser réception). S'il n'y arrive pas, il s'arrête et rouvre à partir de la base ; sinon, un état MLS déjà utilisé serait perdu.
- **La partition d'état**, lue en entier au démarrage par `Client::open`, contient :
  - l'appareil et sa clé ;
  - la réserve de *KeyPackages* ;
  - les groupes, avec les intentions de commit en attente ;
  - les entrées d'OpenMLS ;
  - l'index des messages récents.
- **Une partition d'historique par groupe**, lue à la demande par `read_history`.

### 4. La clé de la base

- **Création** : 32 octets tirés par le cœur (`StoreKey::generate`).
- **Garde** : l'hôte la scelle aussitôt avec le magasin de clés de la plateforme et ne garde que la forme scellée.
  - Dans le navigateur : une clé AES-GCM WebCrypto **non extractible**, rangée dans IndexedDB.
  - Sur Android : le Keystore, dans la liaison mobile (02-13). `SqliteStore::host_value` prévoit l'emplacement de la clé scellée.
- **Exception assumée** : c'est le seul secret qui traverse la liaison. Ce n'est ni une clé d'identité ni une clé MLS, et seul le magasin de clés de la plateforme peut la protéger au repos.

### 5. Éditions et suppressions

- **L'index** : le cœur garde, dans son état, les **1024 derniers messages de chaque groupe** (`EDITABLE_MESSAGES`), avec leur identifiant, leur compte auteur et leur position.
- **La règle** : une édition ou une suppression vise un message de cet index écrit par le même compte. Sinon, elle est ignorée et la raison est rendue (`NotAuthor`, `UnknownTarget`, `AlreadyDeleted`). Le client refuse d'envoyer ce que tous les membres ignoreraient.
- **Les noms d'enregistrement** contiennent l'auteur. Un membre qui reprend l'identifiant du message d'un autre crée un enregistrement distinct et n'écrase jamais le sien.
- **Une suppression** efface le message et son édition de la base, et laisse une pierre tombale (« message supprimé »).

## Limites connues

- **Web** : la clé non extractible empêche un script d'emporter la clé pour déchiffrer ailleurs. Elle n'empêche pas le vol du profil du navigateur sur le disque, puisque la `CryptoKey` y est rangée aussi. C'est la limite du web (principe 9 de la vision).
- **IndexedDB** n'écrase pas toujours tout de suite les données supprimées. Elles restent scellées par la clé de la base.
- **Retour arrière** : remettre une ancienne copie des fichiers de la base fait revivre un ancien état MLS, sans que rien le détecte. À traiter avec les tests de 02-12 ou le durcissement de l'epic 15.
- **Mémoire** : la copie de l'état d'OpenMLS gardée en mémoire n'est pas effacée à la libération, comme le stockage mémoire d'OpenMLS lui-même.

## Conséquences

- `Client::new(device, &key)` et `Client::open(&key, records)` remplacent `Client::new(device)`.
- `Incoming::Message` porte un `effect` : `Added`, `Edited`, `Deleted`, `Nothing` ou `Ignored(..)`.
- Le critère « Android : clé protégée par le Keystore » de la 02-06 se réalise dans la liaison mobile (02-13).
