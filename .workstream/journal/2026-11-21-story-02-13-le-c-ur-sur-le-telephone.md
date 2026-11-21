---
type: Journal
title: "Story 02-13 : le cœur sur le téléphone"
status: draft
created: 2026-11-21
---

# Story 02-13 : le cœur sur le téléphone

Story 02-13 livrée (commit `0e1c755`), vérifiée sur le Xiaomi 2201117TY du propriétaire.

## Ce qui est fait
- **`crates/tessel-api`** : les opérations du cœur sans wasm-bindgen (DTO, `ApiError { code, message }`, `Session`, `PendingLink`). `tessel-wasm` n'en est plus qu'une couche mince ; les fichiers générés pour la page sont identiques à l'octet près, et la WASM grossit de 16 Ko (budget tenu). Codes nouveaux pour l'auto-contrôle : `core:unsupported-ciphersuite`, `core:no-randomness`.
- **`tessel-mobile`** (`packages/core-mobile/rust`, Ream exige le crate dans le paquet qui le déclare) : ABI C (`tessel_configure`, `tessel_call`, `tessel_buf_free`), un appel par opération en MessagePack, `Host` qui tient la base `SqliteStore` et l'écrit après chaque appel, même en échec. La StoreKey est générée au premier lancement, scellée par le rappel de la plateforme (Keystore Android, AES-256-GCM, alias `tessel-store-key-seal`) et gardée scellée par `set_host_value`. Une panique ne traverse pas (`host:panic`).
- **`@tessel/core-mobile`** : module JSI `NativeTessel` (C++), sceau Keystore en Kotlin, `NativeEndpoint`, qui pose le `RemoteCore` du web sur le module natif. L'API est la même par construction.
- **Contrat commun** : les scénarios sortent dans `@tessel/core-web/contract`. Le web les passe en direct et par messages (18), la liaison mobile les passe sur son hôte Rust (9), via `contract-host` (feature de test) et deux FIFO, en appels synchrones comme le JSI.
- **`TextDecoder` UTF-8** pour Hermes, que msgpack instancie à son chargement. Testé contre celui de Node sur entrées malformées et aléatoires.
- **`apps/mobile`** : appli orbit de contrôle. Sur le téléphone, tout passe : version, suite, base Keystore, groupe, message chiffré (402 octets), envoi, conversations, fermeture et réouverture. La base a aussi survécu à un vrai redémarrage de l'appli.

## Les critères
1. Modèle Ream suivi ; tout ce qui manquait est parti en tickets (ci-dessous).
2. Crate ABI C construit pour aarch64 et x86_64 Android (`ream android:build`).
3. Même `TesselCore`, test de contrat commun : vert.
4. Aucune clé privée ne traverse : la StoreKey ne sort jamais du Rust en clair.
5. StoreKey scellée par le Keystore, rangée scellée dans SQLite, feature `sqlite` active.

## Tickets Ream ouverts en chemin
- ream-cli 034 : pas de point d'accroche au démarrage de l'appli pour un module natif.
- ream-cli 035 : l'appli doit déclarer `@swc-node/register`, `@swc/core`, `@react-native/gradle-plugin`, `@react-native/codegen` ; la doc ne les cite pas, et l'erreur sort au fond de Gradle.
- ream-cli 036 : port Metro 8081 codé en dur (ici pris par un conteneur phpMyAdmin d'un autre projet).
- ream-cli 037 : la config Metro générée ignore le monorepo (racine pnpm, `node_modules` de l'appli pour les paquets du workspace, `@babel/runtime` 7).
- ream 117 : `defineConfig` ne type pas le bloc `mobile`.
- orbit 001 : exports avec la seule condition `import`, que Metro ne résout pas depuis npm.
- orbit 002 : pas de `TextDecoder` sous Hermes.

## Contournements locaux (non commités)
Pour lancer l'appli aujourd'hui : Metro démarré à la main sur 8088, `adb reverse tcp:8081 tcp:8088`, et une config Metro locale (condition `import` pour `@c9up/*` seulement, racine du monorepo, `nodeModulesPaths`). Attention : ajouter `import` pour tous les paquets fait prendre les aides ESM de `@babel/runtime` et l'appli meurt au démarrage (« Object is not a function »). `ream android:run` seul ne marchera qu'avec les corrections 036, 037 et orbit 001.

## Hors CI
`check-targets.sh` ne vérifie pas `tessel-mobile` pour Android : SQLite s'y compile en C et demande le NDK, absent de la CI. Les cibles Android sont couvertes par la construction de l'APK.
