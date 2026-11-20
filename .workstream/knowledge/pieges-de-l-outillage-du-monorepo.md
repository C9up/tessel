---
type: Leçon
title: "Pièges de l'outillage du monorepo"
description: "- **pnpm en lieur isolé** (défaut), pas `nodeLinker: hoisted` : ream-cli ne cherche `@swc-node/register` et `@c9up/ream` que dans le `node_modules` de l'app (bu"
tags: []
status: draft
created: 2026-11-20
---

# Pièges de l'outillage du monorepo

- **pnpm en lieur isolé** (défaut), pas `nodeLinker: hoisted` : ream-cli ne cherche `@swc-node/register` et `@c9up/ream` que dans le `node_modules` de l'app (bug 033 de ream-cli).
- **Tests serveur sur un seul fil** (`ream test --threads=1`) : chaque fichier démarre l'app.
- **`podman-compose`**, pas `podman compose` : ce dernier passe par docker-compose, qui exige le socket podman (inactif sur la machine).
- **Biome** formate au style des apps Ream (2 espaces, guillemets simples, sans point-virgule) et ignore Markdown, YAML, `package.json`, `.workstream` et les configs d'agents. Ne jamais lancer `biome check --write` sans ces exclusions : il a déjà reformaté `.mcp.json`, `opencode.json`, `.claude/`, `.gemini/`.
- **`/target` est un lien symbolique** vers `~/.builds/` : dans `.gitignore`, `/target` sans barre finale.
- **`Env.schema.number().optional()`** (forme nœud) : `Env.schema.number.optional` n'existe pas, contrairement à `string.optional`.
- Le hook devguard refuse `rm -rf` et `sed -i` : utiliser `rm -r` sur un chemin vérifié, et Python ou l'outil Edit pour remplacer.

## 2026-11-20

- **SQLite (`rusqlite` avec `bundled`) ne se compile pas pour Android sans le NDK** : `scripts/check-targets.sh` échoue sur `aarch64-linux-android-clang`. C'est pourquoi `rusqlite` est derrière la fonctionnalité `sqlite` de `tessel-core`, coupée par défaut. Les tests l'ont toujours, par une dépendance de développement et `cfg(any(test, feature = "sqlite"))`. La liaison mobile (02-13) l'activera avec le NDK.
- **TypeScript 6 type les octets plus strictement** : WebCrypto et les clés IndexedDB veulent un `Uint8Array<ArrayBuffer>`, pas un `Uint8Array` quelconque. Utiliser le type `Bytes` de `packages/core-web/src/store.ts`, et copier avec `Uint8Array.from(..)` ce qui vient du WASM.
- **Node exécute le TypeScript en retirant seulement les types** (`erasableSyntaxOnly`) : pas de propriété de paramètre (`constructor(readonly x)`), déclarer le champ à part.
- **Le hook devguard refuse `as unknown as`** : pour fabriquer une valeur invalide dans un test, utiliser par exemple un `Proxy`, qui garde le bon type sans être une clé IndexedDB valide.
- **fake-indexeddb** (dépendance de développement de `@tessel/core-web`) teste IndexedDB sous Node. Node sait cloner une `CryptoKey` non extractible, comme le fait IndexedDB.
