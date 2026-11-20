---
type: Epic
title: "tessel-core, le cœur cryptographique"
description: "Le crate Rust unique qui parle MLS, chiffre, déchiffre et garde l'état local, pour le web comme pour le mobile."
tags: [product]
status: draft
order: 2
created: 2026-11-20
---

# tessel-core, le cœur cryptographique

Le crate Rust unique qui parle MLS, chiffre, déchiffre et garde l'état local, pour le web comme pour le mobile.

## Périmètre

- MLS (RFC 9420) via OpenMLS : identités, *KeyPackages*, groupes, messages.
- Base locale chiffrée, file d'envoi hors ligne, chiffrement des médias, codes de sécurité.
- Liaisons WASM (web) et natives (Expo), avec une API asynchrone typée.
- Tests croisés WASM ↔ natif, vecteurs de test MLS, fuzzing des entrées.

## Terminé quand

- Un client web (WASM) et un client Android (natif) échangent des messages dans un même groupe MLS.
- Aucune clé privée ne sort du cœur : l'API ne permet pas de l'exporter.
- Tests, vecteurs de test et fuzzing passent.

## Dépendances

- Epic 01 (squelette du crate et décisions de départ).

## Stories

```workstream
stories
```
