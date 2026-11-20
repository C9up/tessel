---
type: Décision
title: "ADR-003 — Les liaisons du cœur : wasm-bindgen pour le web, uniffi et modules Expo pour le mobile"
description: "tessel-core reste du Rust pur ; deux crates minces l'exposent, tessel-wasm (wasm-bindgen) au web et tessel-ffi (uniffi, Kotlin puis Swift) à un module Expo local ; ubrn est écarté pour l'instant."
tags: [adr, rust, mobile, web]
status: draft
decision_status: "Proposée"
created: 2026-11-20
---

# ADR-003 — Les liaisons du cœur

## Contexte

Toute manipulation de clé vit dans `tessel-core`, en Rust, et doit servir au web (navigateur) comme au mobile (Expo 58, React Native 0.88). La story 02-11 demande la même API des deux côtés.

## Options

| Option | Pour | Contre |
|---|---|---|
| **uniffi-bindgen-react-native (ubrn)** | Un seul crate annoté produit le module natif (JSI) ET le WASM, avec la même API TypeScript | Sa documentation le dit « en développement précoce, à ne pas utiliser en production ». Testé avec React Native 0.75–0.76, alors qu'Expo 58 embarque la 0.88. Ajoute une couche C++/JSI et suppose la structure de `create-react-native-library` |
| **uniffi (Kotlin, Swift) + module Expo local** | uniffi est maintenu par Mozilla et sert dans Firefox mobile. L'API Expo Modules est la voie normale d'Expo pour du natif. Pas de C++ à maintenir | Une couche Kotlin, puis Swift, à écrire à la main pour chaque fonction ; l'API TypeScript est déclarée à la main |
| **wasm-bindgen** (web) | La référence pour Rust → WASM, sans couche supplémentaire | Le CLI doit avoir exactement la version du crate |

## Décision

- **`crates/tessel-core`** : du Rust pur, sans aucune liaison. C'est là que vivent le protocole, les clés et la base locale.
- **`crates/tessel-wasm`** : `wasm-bindgen` (épinglé à la version du CLI, 0.2.129 aujourd'hui), construit par `scripts/build-wasm.sh` dans `packages/core-web`. Le web le chargera dans un Web Worker (story 12-01).
- **`crates/tessel-ffi`** : uniffi 0.32, liaisons Kotlin en mode bibliothèque, puis Swift (story 19-01). `scripts/build-android-core.sh` construit la `.so` avec `cargo-ndk` (arm64-v8a pour les téléphones, x86_64 pour l'émulateur, API 24) et génère le Kotlin. Le module Expo local `apps/mobile/modules/tessel-core` l'expose (JNA 5.17).
- **La même API des deux côtés** est garantie par une interface TypeScript commune (`TesselCore`), implémentée par `@tessel/core-web` et par le module Expo, et par un test de contrat (stories 02-10 et 02-11).
- **ubrn est écarté pour l'instant.** À réévaluer quand il se déclarera prêt pour la production et suivra la version de React Native d'Expo.

## Conséquences

- Chaque fonction exportée par le cœur s'écrit trois fois, très mince : `tessel-wasm`, `tessel-ffi` et le module Kotlin (puis Swift). Ces couches ne décident rien.
- Les macros d'uniffi et de wasm-bindgen engendrent du code `unsafe` : ces deux crates n'héritent pas de `unsafe_code = "deny"` ; `tessel-core` le garde.
- Les fichiers produits (`.so`, Kotlin généré, `packages/core-web/generated`) ne sont pas commités : ils se reconstruisent.
- La chaîne Rust est épinglée (`rust-toolchain.toml`, 1.97.0, cibles WASM et Android) et s'appelle par `scripts/cargo.sh`.

## Sources

- Documentation d'ubrn (guides React Native et web, référence de la ligne de commande et de la configuration), lue le 20.11.2026.
- `create-expo-module` 58.0.5, `expo` 58.0.26 (React Native 0.88.1).
