---
type: Décision
title: "ADR-004 — Les modules du serveur, et ce que chacun sait"
description: "Le serveur Ream est découpé en modules hexagonaux, un par domaine ; chacun possède ses tables et ne sait que ce dont il a besoin pour acheminer, jamais le contenu."
tags: [adr, serveur, metadonnees]
status: draft
decision_status: "Proposée"
created: 2026-11-20
---

# ADR-004 — Les modules du serveur, et ce que chacun sait

## Contexte

Le serveur est un facteur aveugle (vision, principe 1) bâti sur le modèle nadeo : modules à quatre couches, outbox, bus d'événements, une table n'appartient qu'à un module (test d'architecture). Ce que le serveur sait s'inscrit au registre des métadonnées (story 15-02). BlackGate chiffre en base ce qui peut l'être (`deploy/blackgate/columns.yaml`).

## Décision

| Module | Rôle | Ce qu'il sait | Ce qu'il ne sait jamais |
|---|---|---|---|
| `system` | Sonde de vie, informations d'instance | Rien de personnel | — |
| `shared` | Événements du domaine, outbox, briques communes | Le type et la charge des événements, le temps qu'ils transitent (identifiants opaques seulement, ADR-006) | Le contenu d'un message |
| `identity` | Comptes `pseudo@domaine`, appareils, *KeyPackages*, sessions, contacts par lien ou QR, numéro facultatif | Pseudo, clés publiques, appareils, dernière activité au jour près, numéro pseudonymisé (OPRF) | Nom affiché, avatar (chiffrés de bout en bout), carnet d'adresses |
| `delivery` | Boîtes aux lettres, ordre des commits, accusés de remise, purge | En-tête public MLS (groupe, epoch, type), destinataires d'une enveloppe, tailles | Le texte, l'expéditeur réel une fois l'expéditeur scellé (epic 18) |
| `spaces` | Espaces, salons, rôles, permissions, invitations | Identifiants, appartenances, permissions (pour les appliquer) | Noms d'espaces, de salons et de rôles maison (chiffrés de bout en bout) |
| `media` | Blobs chiffrés par le client (`archive`, S3) | Taille, durée de vie, identifiant opaque | Type, nom, contenu |
| `backup` | Sauvegardes chiffrées par la phrase de récupération | Taille, date, nombre de versions | La phrase, la clé, le contenu |
| `moderation` | Signalements, suspensions, fermetures d'espace | Ce que celui qui signale a choisi de transmettre | Tout le reste |
| `push` | Abonnements des appareils, réveils | Jeton ou abonnement de chaque appareil | Pourquoi un appareil est réveillé (la notification est vide) |

Après la v1 : `calls` (signalisation WebRTC, epic 16) et `federation` (epic 20).

## Règles

- Un module ne lit que ses tables (table `OWNER` du test d'architecture, tenue à jour à chaque migration).
- Les modules se parlent par événements (outbox) ou par des ports que fournit le module propriétaire de la donnée.
- Une colonne nouvelle ne garde que du chiffré de bout en bout, un identifiant opaque, ou ce que la base doit comparer ou trier ; elle s'inscrit au registre des métadonnées avec sa raison (story 15-02, ADR-006, qui remplace la règle BlackGate du 20.11.2026).
- Un journal ou une métrique ne contient ni contenu, ni couple expéditeur–destinataire (story 14-02).
