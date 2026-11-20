---
type: Journal
title: "Démarrage de l'epic 01"
status: draft
created: 2026-11-20
---

# Démarrage de l'epic 01

Début du code le 20.11.2026 (« on attaque le projet »).

**01-01 (review)** : dépôt git `main`, monorepo pnpm 12 + turbo, workspace Cargo (`crates/*`, édition 2024), Biome 2.6 (Markdown, YAML, `package.json`, `.workstream` et configs d'agents exclus), squelettes `crates/tessel-core` (`version()`) et `packages/protocol`, README, `AGENTS.md` du projet.
- *Incident* : un premier `biome check --write` a reformaté (indentation seulement) `.claude/settings.local.json`, `.gemini/settings.json`, `.mcp.json` et `opencode.json`. Contenu vérifié identique (JSON valide), fichiers exclus depuis.

**Écart avec l'epic 01 : pas d'`apps/web`.** Dans Ream, l'app sert elle-même ses pages Aurora (nadeo, hugodubois.ch) : la coquille web vit dans `apps/server` (`resources/`, à venir avec 12-01). Deux apps à déployer pour une coquille statique n'apporteraient rien.

**01-02 (review)** : `apps/server` part du gabarit `ream new --template web` (ream-cli 0.3.20, @c9up/ream 0.2.30), refait à la forme nadeo : `app/modules/<module>/{domain,application,infrastructure,http}`, `shared/` (`DomainEvent` horodaté par chronos, outbox + relais + garde de déduplication KeyDB repris de nadeo sans tenant), module `system` (`GET /health`), migration `outbox`. Test d'architecture repris de nadeo (table des propriétaires réduite à `shared: [outbox]`), vérifié en y glissant une violation (domaine qui importe atlas → rouge). 6 tests verts, `tsc` vert, serveur lancé et `/health` répond. Session cookie et `auth_middleware` du gabarit retirés : l'authentification se fera par la clé d'appareil (03-02).

**Lieur pnpm** : `nodeLinker: hoisted` (copié de liste_course) abandonné pour le lieur isolé par défaut. ream-cli ne cherche `@swc-node/register` et `@c9up/ream` que dans `./node_modules` de l'app → ticket `ream-cli` bug 033. Expo supporte le lieur isolé depuis le SDK 54.

**01-03 (en cours)** : `compose.yaml` (Postgres 17, KeyDB) via `podman-compose` (le socket podman est inactif, `podman compose` passe par docker-compose et en a besoin), scripts `pnpm infra` / `infra:down`, ports 3460 / 5460 / 6460 vérifiés libres et notés au README. Restent :
- **BlackGate** : Klinivia le place devant Postgres en TLS mutuel (`scripts/stack.sh`, deux rôles). Question posée au propriétaire. L'app n'en sait rien, donc seule la config de connexion changera.
- **S3** : reporté à 06-01, où `archive` est branché ; un service que rien n'utilise n'apporte que du bruit.

## Suite

**Suite, 01-04 à 01-07 (20.11, après-midi)**

- **Chaîne Rust** : `rust-toolchain.toml` épinglé sur 1.97.0 (cibles WASM et Android). Le PATH de la machine donne le cargo de Fedora (1.99.1) et non celui de rustup, et le shell hérite des variables `CARGO_*` / `TAURI_*` de l'app workstream ; `cargo clippy` partait alors sur la chaîne 1.88 par défaut de rustup. Tout passe désormais par `scripts/cargo.sh` (cargo de rustup).
- **Liaisons (ADR-003)** : `tessel-core` reste du Rust pur ; `tessel-wasm` (wasm-bindgen 0.2.129, version du CLI installé) et `tessel-ffi` (uniffi 0.32.2). Le générateur uniffi vit dans son propre crate `crates/uniffi-bindgen` : avec la fonctionnalité `cli` sur `tessel-ffi`, la bibliothèque livrée tirait askama et `basic-toml` (non maintenu, RUSTSEC-2026-0374). L'avis est ignoré, avec sa raison, dans `deny.toml` et `.cargo/audit.toml` : il ne touche plus que l'outil de build.
- **ubrn écarté pour l'instant** : sa documentation le dit en développement précoce et il est testé avec React Native 0.75–0.76, alors qu'Expo 58 embarque la 0.88.1. Le mobile passe par uniffi (Kotlin) et un module Expo local `apps/mobile/modules/tessel-core` (JNA 5.17), construits par `scripts/build-android-core.sh` (`cargo-ndk` 4.1.2 installé pour l'utilisateur, NDK 27, arm64-v8a + x86_64, API 24).
- **Web** : `packages/core-web`, test Node qui charge le WASM et lit `version()`.
- **App Expo** : `create-expo-app` (SDK 58), licence d'Expo et stubs retirés, iOS retiré du module (story 19-01). L'identifiant `app.tessel.mobile` est **provisoire** : il ne change plus après une publication, à fixer avec le domaine (14-05).
- **Gradle** : une première build est morte avec un démon partagé (7e build, mémoire sous pression : 45 Go utilisés, 43 Go de swap). Relancée avec `--no-daemon` dans un environnement nettoyé (`env -i`).
- **01-05** : `@tessel/protocol` (client comet typé, validation des résultats, `fetchTransport`) et `system.ping` côté serveur ; le test serveur passe par ce client partagé. blackhole refusait `/rpc` (`E_BAD_CSRF_TOKEN`) : `/rpc` est exempté du CSRF, l'API n'ayant aucun cookie (authentification par la clé d'appareil).
- **01-06** : `deny.toml` (licences de l'ADR-005), `cargo deny`, `cargo audit`, rustfmt dans `pnpm lint`. Restent la CI (forge pas encore choisie) et la détection de secrets (aucun outil installé).
- **01-07** : ADR-001 à ADR-005 écrites. BlackGate et `@c9up/ream-cli` ne déclarent aucune licence (relevé dans l'ADR-005).
