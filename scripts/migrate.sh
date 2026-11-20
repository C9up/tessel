#!/usr/bin/env bash
# =============================================================================
# Joue les migrations sous le propriétaire du schéma, jamais sous le rôle de
# l'application : `app` ne possède rien, il ne peut donc pas se rendre ce
# qu'une migration lui a retiré (repris de Klinivia).
#
#   scripts/migrate.sh [migration:run | migration:rollback | migration:status …]
#
# Lit DB_OWNER_USER et DB_OWNER_PASSWORD dans apps/server/.env (écrits par
# scripts/stack.sh start) et les passe à la commande comme DB_USER et
# DB_PASSWORD, qui l'emportent sur .env. Le reste (hôte, certificats) est celui
# de l'application.
# =============================================================================
set -euo pipefail

SERVER="$(cd "$(dirname "${BASH_SOURCE[0]}")/../apps/server" && pwd)"
read_env() { # read_env <clé> : la valeur dans .env, ou rien
    python3 -I -c 'import sys
for line in open(sys.argv[1]):
    key, sep, value = line.rstrip("\n").partition("=")
    if sep and key == sys.argv[2]:
        print(value)
        break' "$SERVER/.env" "$1"
}

owner="$(read_env DB_OWNER_USER)"
owner_password="$(read_env DB_OWNER_PASSWORD)"
[ -n "$owner" ] || { echo "DB_OWNER_USER absent de apps/server/.env (scripts/stack.sh start)" >&2; exit 2; }

cd "$SERVER"
DB_USER="$owner" DB_PASSWORD="$owner_password" exec "$SERVER/node_modules/.bin/ream" "${@:-migration:run}"
