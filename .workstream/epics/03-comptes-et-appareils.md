---
type: Epic
title: "Comptes et appareils"
description: "Créer un compte sans numéro, s'authentifier avec la clé de son appareil, ajouter un second appareil par QR et révoquer un appareil perdu."
tags: [product]
status: draft
order: 3
created: 2026-11-20
---

# Comptes et appareils

Créer un compte sans numéro, s'authentifier avec la clé de son appareil, ajouter un second appareil par QR et révoquer un appareil perdu.

## Périmètre

- Module serveur `identity` : comptes `pseudo@domaine`, appareils, annuaire de *KeyPackages*.
- Authentification par signature de la clé d'appareil (pas de mot de passe) ; sessions par `warden`.
- Ajout d'un appareil par QR, liste et révocation des appareils, profil chiffré, suppression de compte.

## Terminé quand

- Un compte créé sur le web ajoute un appareil Android par QR, et l'inverse.
- Un appareil révoqué perd son accès et sort de tous ses groupes.

## Dépendances

- Epic 02 (identités et KeyPackages du cœur).

## Stories

```workstream
stories
```
