---
type: Leçon
title: "CI GitHub Actions"
description: "`"
tags: [ci, github, gitleaks]
status: draft
created: 2026-11-20
---

# CI GitHub Actions

`.github/workflows/check.yml` (story 01-06), dépôt public `C9up/tessel` : les runners standard ne consomment pas de minutes facturées pour un dépôt public, mais le propriétaire veut une CI économe.

- **Déclencheurs** : push sur `main`, pull requests, lancement manuel. Les changements qui ne touchent que `.workstream/`, du Markdown, `.claude/` ou `.agents/` ne lancent rien. `concurrency` annule l'exécution dépassée.
- **Durées mesurées** (première exécution, cache froid) : `pnpm check` 3 min 24 s, gitleaks 5 s.
- **Job `check`** :
  - la même pile qu'en local : `scripts/stack.sh` (PostgreSQL en TLS mutuel, podman, présent sur ubuntu-24.04) et `podman-compose` (installé par pipx) pour KeyDB ;
  - puis `ream generate:key`, les migrations, le build WASM (`@tessel/core-web build`, que les tests du cœur web chargent) et `pnpm check`.
- **Job `secrets`** :
  - gitleaks 8.30.1 sur tout l'historique, archive vérifiée par son SHA-256 épinglé ;
  - `.gitleaks.toml` exclut la seule règle `generic-api-key` sur `Cargo.lock` et `pnpm-lock.yaml`, parce que le crate `libcrux-secrets` la déclenche (faux positif) ;
  - un commit de documentation seule n'est scanné qu'au prochain commit de code.
- **Actions épinglées par commit**, pas par étiquette. Pour en monter une : `gh api repos/<owner>/<action>/commits/<tag> -q .sha`.
- **gitleaks ne peut pas tourner sur la machine de dev** depuis l'agent (exécuter un binaire téléchargé est refusé) : on vérifie dans la CI.
