---
type: Décision
title: "ADR-003 — Les liaisons du cœur : un cœur Rust, une liaison par hôte, sur le modèle de Ream"
description: "tessel-core est un crate Rust sans hôte ; le web l'atteint par WASM (tessel-wasm), le mobile par la liaison que la stack mobile de Ream définit (ABI C + JSI), sans React Native, Expo ni modules Expo."
tags: [adr, rust, mobile, web]
status: draft
decision_status: "Proposée — révisée le 20.11.2026 : le mobile suit l'ADR-009 de Ream (Tessel, app pilote de l'epic 73)"
created: 2026-11-20
---

# ADR-003 — Les liaisons du cœur

## Contexte

Toute manipulation de clé vit dans `tessel-core`, en Rust, et sert au web comme au mobile ; la story
02-11 demande la même API des deux côtés.

Le 20.11.2026, Ream a fixé sa direction mobile (`ream:decisions/adr-009-one-ecosystem-from-server-to-phone.md`,
epic 73) : un composant aurora écrit une fois et rendu par trois hôtes (HTML sur le serveur, DOM dans
le navigateur, vues natives sur le téléphone), **sans React ni Expo**, et **un cœur Rust par domaine,
trois liaisons** (NAPI pour Node, WASM pour le navigateur, JSI par une ABI C pour le mobile), la même
API TypeScript au-dessus. Ces liaisons remplacent les modules Expo. Le propriétaire a fait de Tessel
l'**app pilote** de cette stack.

## Historique

Une première version de cette décision (le même jour, avant l'ADR-009 de Ream) passait par uniffi
(Kotlin) dans un module Expo local, et a prouvé que le cœur tournait sur Android (story 01-04, APK
release sur émulateur : « tessel-core 0.1.0 »). Cette voie a été retirée : elle ajoutait un second
écosystème (React Native, Expo, Kotlin écrit à la main) que l'ADR-009 de Ream écarte. ubrn
(uniffi-bindgen-react-native) avait été écarté pour la même raison et parce qu'il se dit encore
pas prêt pour la production.

## Décision

- **`crates/tessel-core`** : un crate Rust sans hôte, sans aucune liaison : protocole, clés, base locale.
- **Web : `crates/tessel-wasm`** (wasm-bindgen, épinglé à la version du CLI, 0.2.129), construit par
  `scripts/build-wasm.sh` dans `packages/core-web`, chargé dans un Web Worker (story 12-01).
- **Mobile : la liaison que la stack mobile de Ream définit** (ABI C + JSI, sur le modèle de
  `atom:epics/03-a-mobile-binding-for-the-rust-engine.md`). Tessel l'adopte telle quelle dès qu'elle
  existe, plutôt que d'inventer la sienne ; ce qui manque est du travail pour Ream (epic 73), pas un
  contournement dans Tessel.
- **Node (NAPI)** : seulement si le serveur ou les tests en ont besoin (par exemple un test
  d'interopérabilité qui joue un client dans Node). Le serveur, lui, ne manipule jamais de clé.
- **La même API TypeScript** au-dessus de chaque liaison (`TesselCore`), vérifiée par un test de
  contrat (stories 02-10 et 02-13).
- **L'interface s'écrit une fois** en aurora et nebula (epic 12) ; le mobile la rend par l'hôte natif
  de Ream (epic 13).

## Conséquences

- Les couches de liaison restent très minces et ne décident rien. Leurs macros engendrent du code
  `unsafe` : elles n'héritent pas de `unsafe_code = "deny"` ; `tessel-core` le garde.
- `rust-toolchain.toml` garde les cibles Android (`aarch64-linux-android`, `x86_64-linux-android`) pour la
  liaison mobile à venir.
- Le calendrier mobile de Tessel suit celui de l'epic 73 de Ream ; la première étape réussie (web +
  Android) en dépend.
- `tessel-core` doit charger sur un moteur JS mobile sans `node:`, sans DOM, sans `await` de premier
  niveau du côté TypeScript, comme l'ADR-009 de Ream l'exige d'aurora.

## Sources

- `ream:decisions/adr-009-one-ecosystem-from-server-to-phone.md` et `ream:epics/73-mobile-apps-on-the-ream-ecosystem-without-react.md` (20.11.2026).
- Documentation d'ubrn, lue le 20.11.2026.
