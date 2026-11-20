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
