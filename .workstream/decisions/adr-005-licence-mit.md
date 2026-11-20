---
type: Décision
title: "ADR-005 — Licence MIT"
description: "Tessel est publié sous licence MIT, choix du propriétaire ; les licences des dépendances sont compatibles et cargo deny tient la liste des licences admises."
tags: [adr, licence]
status: draft
decision_status: "Acceptée"
created: 2026-11-20
---

# ADR-005 — Licence MIT

## Contexte

Tessel est open source (cadrage du 15.11.2026). Le propriétaire a choisi la licence le 20.11.2026, parmi AGPL-3.0, Apache-2.0, MIT et GPL-3.0.

## Décision

**MIT.** Le fichier `LICENSE` est à la racine. Les paquets npm et les crates déclarent `MIT`.

Conséquence assumée : n'importe qui peut reprendre le code, y compris dans un service fermé. La confiance repose sur le code publié et sur l'audit externe (story 15-05), pas sur la licence.

## Compatibilité des dépendances (relevé du 20.11.2026)

| Dépendance | Licence | Compatible |
|---|---|---|
| OpenMLS, `openmls_rust_crypto` | MIT | Oui |
| wasm-bindgen | MIT / Apache-2.0 | Oui |
| Paquets `@c9up/*` utilisés (ream, atlas, blackhole, chronos, quasar, spectrum, helix) | MIT | Oui |
| `@c9up/ream-cli` | Aucune déclarée | Outil de développement, non distribué ; à signaler à ream-cli |
| BlackGate | Aucune déclarée (ni `Cargo.toml`, ni `LICENSE`) | Service séparé, non lié au code de Tessel ; à clarifier avant d'en livrer une image avec Tessel (story 14-04) |
| PostgreSQL / KeyDB | PostgreSQL / BSD-3-Clause | Oui (services séparés) |
| libsignal | AGPL-3.0 | **Écarté** (ADR-001) |

Le mobile ne dépend ni de React Native ni d'Expo (ADR-003) : sa liaison viendra de la stack mobile de Ream, dont les licences seront relevées à son arrivée.

## Conséquences

- `cargo deny` (story 01-06) admet : MIT, Apache-2.0, MPL-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, CC0-1.0. Toute autre licence fait échouer `pnpm check`.
