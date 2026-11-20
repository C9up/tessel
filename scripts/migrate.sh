#!/usr/bin/env bash
# =============================================================================
# Runs the migrations as the schema owner, never as the application's role:
# `app` owns nothing, so it cannot give itself back what a migration took from
# it (taken from Klinivia).
#
#   scripts/migrate.sh [migration:run | migration:rollback | migration:status …]
#
# Reads DB_OWNER_USER and DB_OWNER_PASSWORD from apps/server/.env (written by
# scripts/stack.sh start) and passes them to the command as DB_USER and
# DB_PASSWORD, which take precedence over .env. The rest (host, certificates) is
# the application's.
# =============================================================================
set -euo pipefail

SERVER="$(cd "$(dirname "${BASH_SOURCE[0]}")/../apps/server" && pwd)"
read_env() { # read_env <key>: the value in .env, or nothing
    python3 -I -c 'import sys
for line in open(sys.argv[1]):
    key, sep, value = line.rstrip("\n").partition("=")
    if sep and key == sys.argv[2]:
        print(value)
        break' "$SERVER/.env" "$1"
}

owner="$(read_env DB_OWNER_USER)"
owner_password="$(read_env DB_OWNER_PASSWORD)"
[ -n "$owner" ] || { echo "DB_OWNER_USER missing from apps/server/.env (scripts/stack.sh start)" >&2; exit 2; }

cd "$SERVER"
DB_USER="$owner" DB_PASSWORD="$owner_password" exec "$SERVER/node_modules/.bin/ream" "${@:-migration:run}"
