---
type: Journal
title: "Story 02-13 bloquée : la liaison mobile attend Ream"
status: draft
created: 2026-11-21
---

# Story 02-13 bloquée : la liaison mobile attend Ream

La 02-13 doit suivre le modèle de liaison mobile de Ream, sans contournement dans Tessel. Ce modèle n'existe pas encore :
- `ream:` ADR-009, décision 6 : l'hôte natif est tranché (hôte Rust sur les vues natives, Hermes comme moteur JS ; spikes 73-01 et 73-07) ;
- mais la façon de brancher un moteur Rust sur cet hôte (ABI C + JSI) revient à la story atom 03-01, encore « à faire ». Le prototype `labs/rn-jsi-spike/rust/atom-ffi` ne passe que des chaînes, par des fonctions sans état.

Ce dont Tessel aura besoin est consigné dans le journal d'atom (`packages/atom/.workstream/journal/2026-11-21-what-tessel-needs-from-the-mobile-binding.md`, pas encore commité dans le dépôt Ream) : des octets dans les deux sens, un objet avec un état, des valeurs structurées, des appels hors du fil JS, des erreurs à code stable, et l'accès au Keystore Android depuis Rust.

La 02-13 reprend quand atom 03-01 est faite. En attendant, la 02-12 avance sur ce qui ne dépend pas du téléphone : vecteurs de test MLS, fuzzing, et un test croisé entre le cœur compilé en WASM et le cœur natif.
