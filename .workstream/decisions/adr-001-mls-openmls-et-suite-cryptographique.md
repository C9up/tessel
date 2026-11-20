---
type: Décision
title: "ADR-001 — MLS via OpenMLS, et la suite cryptographique"
description: "Le chiffrement de bout en bout de Tessel repose sur MLS (RFC 9420), implémenté par OpenMLS avec son fournisseur crypto en Rust pur, sur la suite 0x0001."
tags: [adr, crypto]
status: draft
decision_status: "Proposée"
created: 2026-11-20
---

# ADR-001 — MLS via OpenMLS, et la suite cryptographique

## Contexte

La vision fixe trois exigences : pas de cryptographie maison (principe 3), un vrai multi-appareil où chaque appareil est membre du groupe (principe 5), et des espaces de quelques milliers de membres. Le cœur `tessel-core` doit compiler en WASM et pour Android, puis pour iOS. Le code est sous licence MIT (ADR-005).

## Options

| Option | Pour | Contre |
|---|---|---|
| **Protocole Signal** (libsignal) | Éprouvé à très grande échelle | AGPL-3.0, incompatible avec une distribution MIT du client. Groupes par *sender keys*, sans accord de groupe vérifiable, et coûteux au-delà de quelques centaines de membres |
| **MLS via OpenMLS** | RFC 9420, Rust, MIT. Fonctionnalité `js` pour le WASM, stockage SQLite fourni, suites post-quantiques en brouillon derrière une fonctionnalité | Version 0.x : l'API bouge encore entre versions mineures |
| **MLS via mls-rs** (AWS) | RFC 9420, Rust, Apache-2.0 / MIT | Communauté plus petite autour du WASM et de l'interopérabilité MIMI |

## Décision

- **MLS (RFC 9420) via OpenMLS 0.9.x**, épinglé à une version publiée. Il demande Rust 1.91 ; le dépôt est sur la 1.97.
- **Fournisseur crypto : `openmls_rust_crypto`** (MIT), en Rust pur. Il compile pour le WASM comme pour Android sans chaîne C. `openmls_libcrux_crypto` (code vérifié formellement) reste une alternative, à mesurer plus tard.
- **Suite `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519` (0x0001)**. C'est la suite que RFC 9420 rend obligatoire : elle maximise l'interopérabilité, ce qui compte pour la fédération (MIMI). `0x0003` (ChaCha20-Poly1305) n'aurait d'intérêt que sur des appareils sans AES matériel, et les ARM visés en ont tous.
- **Post-quantique : à réévaluer avant toute ouverture au public**, avec l'audit externe (story 15-05). La menace « récolter maintenant, déchiffrer plus tard » vise justement des messageries. OpenMLS propose des suites X-Wing en brouillon (`draft-ietf-mls-pq-ciphersuites`), mais un brouillon n'a pas sa place dans la v1.

## Conséquences

- La story 02-01 vérifie la compilation sur les trois cibles et `cargo deny` sur l'arbre d'OpenMLS.
- Chaque montée de version d'OpenMLS est une story, avec les vecteurs de test MLS officiels (02-12).
- La taille maximale d'un espace dépendra du coût mesuré des commits (étude 10-01).

## Sources

- RFC 9420, *The Messaging Layer Security (MLS) Protocol*.
- `cargo info openmls@0.9.2` : licence MIT, rust-version 1.91.0, fonctionnalités `js`, `sqlite-provider`, `libcrux-provider`, `draft-ietf-mls-pq-ciphersuites` (relevé le 20.11.2026).
