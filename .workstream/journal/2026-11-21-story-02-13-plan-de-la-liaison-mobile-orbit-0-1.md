---
type: Journal
title: "Story 02-13 : plan de la liaison mobile (orbit 0.1.0)"
status: draft
created: 2026-11-21
---

# Story 02-13 : plan de la liaison mobile (orbit 0.1.0)

Décision du propriétaire (21.11) : « oui », la 02-13 avant la fin de l'epic 05. Son téléphone (Xiaomi 2201117TY) est branché en mode développeur et visible par `adb`.

## Le modèle de Ream, tel qu'orbit 0.1.0 le montre
- Un paquet déclare ses parties natives dans `package.json`, sous `reamNative` :
  - `rust` : crates compilés en `staticlib` ;
  - `android.java` : sources Kotlin ;
  - `android.cmake` : `file` et fonction `link` ;
  - `android.cxxModules` : modules C++ (TurboModule) ;
  - `specs` : spec codegen.
- `ream android:run` / `android:build` :
  - génèrent `.ream/android` ;
  - compilent les crates Rust pour chaque ABI avec le NDK ;
  - branchent les modules C++ ;
  - installent par `adb`, avec Metro pour le rechargement à chaud.
- L'application déclare un bloc `mobile` dans `reamrc.ts` et un point d'entrée qui appelle `run(App)` de `@c9up/orbit/react-native`.
- ABI d'orbit : fonctions `extern "C"`, JSON en entrée, JSON en sortie, dans un `OrbitBuf { ptr, len, cap }` libéré par `*_buf_free`. Appels synchrones sur le fil JS (JSI). JNI (fbjni) pour atteindre Kotlin.

## Plan pour Tessel
1. **Crate `tessel-api`**, extrait de `tessel-wasm` :
   - les DTO serde ;
   - les codes d'erreur (`ApiError { code, message }`) ;
   - une `Session` qui porte le `Client`, le sceau des médias et la liaison en attente, avec une méthode par opération de `TesselCore`.

   Aucune dépendance à `wasm_bindgen`. `tessel-wasm` devient une couche mince au-dessus, sans changement visible : même contrat, les 34 tests web doivent rester verts.
2. **Crate `tessel-mobile`** (`staticlib`, ABI C) :
   - un seul point d'entrée `tessel_call(method, args JSON) -> JSON` qui aiguille vers la `Session` ;
   - la base SQLite dans Rust (fonctionnalité `sqlite`), persistée après chaque appel (ADR-007) ;
   - la `StoreKey` scellée par un rappel de la plateforme (Keystore Android, clé AES non exportable) et rangée scellée par `SqliteStore::set_host_value`.

   Tests sur l'hôte Linux avec un faux sceau ; compilation croisée pour `aarch64-linux-android` et `x86_64-linux-android`.
3. **Paquet `@tessel/core-mobile`** (`reamNative`) :
   - spec `NativeTessel.ts` ;
   - module C++ `NativeTessel` (JSI → `tessel_call`, JNI → Keystore) ;
   - Kotlin `TesselKeystore` ;
   - classe TS `MobileCore` qui implémente `TesselCore` : octets ↔ JSON, erreurs `TesselError` avec les mêmes codes.
4. **Preuve sur le téléphone** : une petite appli `apps/mobile` (bloc `mobile`, orbit) qui déroule sur l'appareil les scénarios du test de contrat (créer un compte, groupe, message, redémarrage) et affiche le résultat. Lancée avec `ream android:run`.

Tout manque côté Ream devient un ticket dans le `.workstream` du paquet concerné, jamais un contournement dans Tessel (critère de la story).
