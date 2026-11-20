---
type: Décision
title: "ADR-002 — Une identité fédérable dès le premier jour : pseudo@domaine"
description: "Un compte Tessel s'écrit pseudo@domaine, et un appareil est désigné par son compte plus un identifiant d'appareil, pour que la fédération puisse venir sans rien migrer."
tags: [adr, identite, federation]
status: draft
decision_status: "Proposée"
created: 2026-11-20
---

# ADR-002 — Une identité fédérable dès le premier jour : pseudo@domaine

## Contexte

La fédération est prévue mais activée après la v1 (vision, principe 8). Si une identité ne nommait pas son serveur, la fédération obligerait plus tard à réécrire chaque *credential* MLS, et donc à refaire tous les groupes. Le numéro de téléphone est facultatif, donc il ne peut pas servir d'identité.

## Décision

- **Un compte s'écrit `pseudo@domaine`.** `domaine` est le nom de domaine de l'instance (variable `APP_DOMAIN`). Il ne change plus une fois qu'un compte existe : le serveur refuse de démarrer si ce domaine diffère de celui qu'il a enregistré.
- **Pseudo** : 3 à 32 caractères parmi `a-z`, `0-9`, `.`, `_`, `-`, en minuscules, avec une lettre en premier. Pas d'Unicode, pour éviter les homoglyphes (un `а` cyrillique qui imite un `a`). Le nom affiché, lui, est libre et chiffré (story 03-06).
- **Un appareil est désigné par `pseudo@domaine` et un identifiant d'appareil** (128 bits aléatoires). Son *credential* MLS porte les deux et est signé par la clé d'identité du compte (story 02-02). Tout membre d'un groupe peut ainsi vérifier qu'un appareil appartient au compte annoncé.
- **La découverte d'un compte distant passera par son domaine** (piste IETF MIMI, epic 20). Rien n'est construit qui supposerait un seul serveur.

## Conséquences

- Les tables qui désignent un compte gardent le domaine, ou une clé qui y mène, même si la v1 n'en connaît qu'un.
- Un pseudo libéré reste réservé un temps contre l'usurpation (story 03-07).
- Changer de domaine revient à migrer une instance : ce cas ne fait pas partie de la v1.
