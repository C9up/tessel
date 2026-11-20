---
type: Leçon
title: "Base de données à travers BlackGate"
description: "PostgreSQL n'est joignable qu'à travers BlackGate en TLS mutuel (`sslmode=verify-full`, `databaseUrl()`), en dev comme en prod (décision du propriétaire, 20"
tags: []
status: draft
created: 2026-11-20
---

# Base de données à travers BlackGate

PostgreSQL n'est joignable qu'à travers BlackGate en TLS mutuel (`sslmode=verify-full`, `databaseUrl()`), en dev comme en prod (décision du propriétaire, 20.11.2026). Repris de Klinivia :
- `pnpm infra` lance `scripts/stack.sh start` (PostgreSQL 17 TLS seulement, BlackGate, certificats et mots de passe neufs dans `.stack/`) et écrit les lignes DB_* de `apps/server/.env`.
- Deux rôles : `tessel_owner` joue les migrations (`pnpm --filter @tessel/server migrate` → `scripts/migrate.sh`), `app` fait tourner l'application et reçoit ses droits de la migration `grant_application_role`. Pas d'`autoRun` des migrations.
- Une colonne à chiffrer se déclare dans `deploy/blackgate/columns.yaml` AVANT sa migration, en `text` (la colonne porte le chiffré), puis `pnpm infra:reload`. Toute colonne en clair va dans `deploy/blackgate/clear-columns.yaml` avec sa raison.
- Les tests lisent PostgreSQL en direct (port 5461, `connectDirectDb()`) pour prouver qu'une colonne chiffrée ne contient que du chiffré. Un tel test doit vérifier que la ligne existe : `String(undefined)` ne contient pas le marqueur non plus.
- Vérifié : `FOR UPDATE SKIP LOCKED`, `make_interval`, `RETURNING` passent à travers BlackGate.
