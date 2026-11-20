---
type: Leçon
title: "Base de données en TLS mutuel"
description: "PostgreSQL n'est joignable qu'en TLS mutuel (`sslmode=verify-full` et certificat client, `databaseUrl()`), en dev, en CI et en prod"
tags: [base-de-donnees, tls, infra]
status: draft
created: 2026-11-20
---

# Base de données en TLS mutuel

PostgreSQL n'est joignable qu'en TLS mutuel (`sslmode=verify-full` et certificat client, `databaseUrl()`), en dev, en CI et en prod. Pas de BlackGate depuis l'ADR-006 (20.11.2026) : produit propriétaire, retiré.

- `pnpm infra` lance `scripts/stack.sh start` : certificats ECDSA P-256 neufs par `openssl` (CA, serveur `localhost`, client), PostgreSQL 17 sous podman en TLS seulement (`hostssl … scram-sha-256 clientcert=verify-ca`, `hostnossl … reject`), mots de passe neufs ; écrit les lignes DB_* de `apps/server/.env`. Port 5460.
- Deux rôles : `tessel_owner` joue les migrations (`pnpm --filter @tessel/server migrate` → `scripts/migrate.sh`), `app` fait tourner l'application et reçoit ses droits de la migration `grant_application_role`. Pas d'`autoRun` des migrations.
- `clientcert=verify-ca` et pas `verify-full` : un seul certificat client sert les deux rôles, et `verify-full` exigerait que son CN soit le nom du rôle.
- Le test `outbox.integration.test.ts` prouve qu'une connexion sans TLS, ou en TLS sans certificat client, est refusée ; un témoin vérifie que la même URL avec les quatre paramètres TLS se connecte, pour que les refus viennent bien du TLS.
- Une colonne nouvelle ne garde que du chiffré de bout en bout, un identifiant opaque ou ce que la base doit comparer ou trier, et s'inscrit au registre des métadonnées (15-02).
- Vérifié : `FOR UPDATE SKIP LOCKED`, `make_interval`, `RETURNING` passent par atlas en TLS.
- Pour passer de l'ancienne pile à celle-ci : tuer le proxy BlackGate (`.stack/proxy.pid`) avant `scripts/stack.sh stop`, la nouvelle version ne le connaît plus.
