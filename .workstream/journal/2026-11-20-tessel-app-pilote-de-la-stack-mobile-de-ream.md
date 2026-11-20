---
type: Journal
title: "Tessel, app pilote de la stack mobile de Ream"
status: draft
created: 2026-11-20
---

# Tessel, app pilote de la stack mobile de Ream

Le 20.11.2026, le propriétaire signale qu'il construit une stack complète avec Ream pour remplacer React Native et Expo (`ream:decisions/adr-009-one-ecosystem-from-server-to-phone.md`, `ream:epics/73-mobile-apps-on-the-ream-ecosystem-without-react.md`, créés le même jour). Choix : **Tessel devient l'app pilote de l'epic 73**.

**Ce que dit l'ADR-009 de Ream** : un composant aurora écrit une fois, rendu par trois hôtes (HTML serveur, DOM, vues natives) ; un cœur Rust par domaine et trois liaisons (NAPI, WASM, JSI par une ABI C) qui remplacent les modules Expo ; comet et photon comme couche de données du mobile ; ream-cli pour les builds et la signature ; nova étendu à FCM et APNs ; hôte natif en Rust si la mesure le permet, Fabric sinon.

**Changements dans Tessel**
- Code : `apps/mobile` (Expo), `crates/tessel-ffi`, `crates/uniffi-bindgen`, `scripts/build-android-core.sh` et `scripts/check-android.sh` supprimés. L'exception RUSTSEC-2026-0374 (`basic-toml`) disparaît avec le générateur uniffi. Les cibles Android restent dans `rust-toolchain.toml` pour la liaison mobile à venir.
- ADR-003 révisée (un cœur, une liaison par hôte, la liaison mobile de Ream adoptée telle quelle) ; ADR-005 sans uniffi, JNA ni Expo ; vision alignée (interface unique aurora / nebula, Tessel pilote de l'epic 73, travail pour Ream).
- Archivées dans `archive/stories/` : 02-11 (liaisons Expo) et 13-01 à 13-08 (app Expo et ses écrans Android). Créées : **02-13** (liaison mobile du cœur sur la stack Ream), **13-10** (coquille mobile), **13-11** (les écrans de l'epic 12 en vues natives, sans fichier propre au mobile), **13-12** (capacités natives : caméra, galerie, QR, carnet), **13-13** (notifications FCM). Dépendances repointées (02-12, 13-09, 18-03, 19-01, 19-02, 21-01). Epics 01, 02, 12, 13 et 19 réécrites en conséquence.
- Stories en review dont un critère parlait d'Expo : 01-04 (review « critère mobile retiré, repris par 02-13 et 13-10 »). 01-03 (« l'app Expo se lance à part »), 01-05 (« le client tourne dans Expo ») et 01-07 (« ubrn pour Expo », décision prise puis révisée dans l'ADR-003) gardent leur texte d'origine.
- La première étape réussie (web + Android) dépend désormais de l'avancement de l'epic 73.
- L'APK de test `app.tessel.mobile` est resté sur l'émulateur Pixel_7 du propriétaire (émulateur arrêté au moment de la désinstallation).

**Côté nova** : l'epic nova 03 (push natif) est désormais portée par l'ADR-009 de Ream (« nova grows from Web Push to FCM and APNs ») ; sa story 03-01 (ADR qui élargit nova) est tranchée sur le fond par cet ADR.
