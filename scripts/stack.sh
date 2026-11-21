#!/usr/bin/env bash
# =============================================================================
# Development stack: PostgreSQL over mutual TLS, as in production, and KeyDB, both
# in one compose project (compose.yaml), so they start, stop and show up together.
#
#   scripts/stack.sh start   # certificates, passwords, the compose project, DB_* lines of .env
#   scripts/stack.sh stop    # stops the project and erases .stack/
#
# The application reaches PostgreSQL only over TLS with `sslmode=verify-full` and a
# client certificate: no plaintext shortcut, not even in development. `start` writes
# the DB_* lines into apps/server/.env (fresh passwords and certificates on every
# start); the port is fixed.
#
# What the database keeps is already end-to-end encrypted by the devices, or an
# opaque identifier (ADR-006): the stack encrypts the connection, not the columns.
#
# Two roles: `tessel_owner` owns the schema and runs the migrations
# (scripts/migrate.sh); `app`, which the application runs as, owns nothing and
# gets its rights from the migrations.
#
# Prerequisites: podman, podman-compose, python3, openssl. The CI runs the same script.
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$ROOT/.stack"
ENV_FILE="$ROOT/apps/server/.env"
CONTAINER=tessel-postgres
PG_PORT="${STACK_PG_PORT:-5460}"

compose() {
    podman-compose -f "$ROOT/compose.yaml" "$@"
}

stop() {
    compose down >/dev/null 2>&1 || true
    # A PostgreSQL started outside compose, by an earlier version of this script.
    podman rm -f "$CONTAINER" >/dev/null 2>&1 || true
    python3 -I -c 'import shutil, sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$WORK"
}

# A CA, a server certificate for localhost and one client certificate, all ECDSA P-256.
certificates() { # certificates <dir>
    local dir="$1"
    mkdir -p "$dir"
    openssl req -x509 -new -nodes -newkey ec -pkeyopt ec_paramgen_curve:P-256 -days 30 \
        -subj "/CN=Tessel dev CA" -keyout "$dir/ca.key" -out "$dir/ca.crt" 2>/dev/null
    for name in server client; do
        openssl req -new -nodes -newkey ec -pkeyopt ec_paramgen_curve:P-256 \
            -subj "/CN=$([ "$name" = server ] && echo localhost || echo app)" \
            -keyout "$dir/$name.key" -out "$dir/$name.csr" 2>/dev/null
    done
    printf 'subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\n' > "$dir/server.ext"
    printf 'extendedKeyUsage=clientAuth\n' > "$dir/client.ext"
    for name in server client; do
        openssl x509 -req -in "$dir/$name.csr" -CA "$dir/ca.crt" -CAkey "$dir/ca.key" \
            -CAcreateserial -days 30 -extfile "$dir/$name.ext" -out "$dir/$name.crt" 2>/dev/null
    done
    rm -f "$dir"/*.csr "$dir"/*.ext "$dir/ca.key" "$dir/ca.srl"
    chmod 600 "$dir"/*.key
}

# Replaces (or adds) the DB_* keys in apps/server/.env, leaving the rest untouched.
update_env() {
    [ -f "$ENV_FILE" ] || cp "$ROOT/apps/server/.env.example" "$ENV_FILE"
    python3 -I - "$ENV_FILE" "$WORK/env" <<'PY'
import sys
target, source = sys.argv[1], sys.argv[2]
new = dict(l.rstrip("\n").split("=", 1) for l in open(source) if "=" in l and not l.startswith("#"))
lines, seen = [], set()
for line in open(target):
    key = line.split("=", 1)[0]
    if "=" in line and key in new:
        lines.append(f"{key}={new[key]}\n"); seen.add(key)
    else:
        lines.append(line)
lines += [f"{k}={v}\n" for k, v in new.items() if k not in seen]
open(target, "w").writelines(lines)
PY
    chmod 600 "$ENV_FILE"
}

start() {
    [ -d "$WORK" ] && { echo "$WORK exists: run scripts/stack.sh stop first" >&2; exit 2; }
    mkdir -p "$WORK/pg"
    chmod 700 "$WORK"
    local password owner_password
    password="$(openssl rand -hex 16)"
    owner_password="$(openssl rand -hex 16)"

    certificates "$WORK/certs"

    # PostgreSQL: TLS only over TCP, SCRAM, and a client certificate signed by the CA.
    cp "$WORK/certs/server.crt" "$WORK/certs/server.key" "$WORK/certs/ca.crt" "$WORK/pg/"
    chmod 644 "$WORK/pg/server.crt" "$WORK/pg/server.key"
    cat > "$WORK/pg/init.sql" <<SQL
CREATE ROLE tessel_owner LOGIN PASSWORD '$owner_password';
CREATE ROLE app LOGIN PASSWORD '$password';
CREATE DATABASE tessel OWNER tessel_owner;
\\c tessel
REVOKE ALL ON SCHEMA public FROM PUBLIC;
GRANT USAGE ON SCHEMA public TO app;
SQL
    cat > "$WORK/pg/hba.conf" <<'HBA'
local     all all              trust
hostssl   all all 0.0.0.0/0    scram-sha-256 clientcert=verify-ca
hostnossl all all 0.0.0.0/0    reject
HBA
    # The superuser's password: the entrypoint wants one; nothing uses it.
    openssl rand -hex 16 > "$WORK/pg/superuser"
    chmod 644 "$WORK/pg/superuser"
    # Both containers, as one compose project: compose.yaml reads .stack/pg.
    STACK_PG_PORT="$PG_PORT" compose up -d >/dev/null
    # The entrypoint runs init.sql on a temporary server, then starts the real one.
    local ready=0
    for _ in $(seq 1 120); do
        if podman logs "$CONTAINER" 2>&1 | grep -q 'PostgreSQL init process complete' \
            && podman exec "$CONTAINER" pg_isready -h 127.0.0.1 -U app -d tessel >/dev/null 2>&1; then
            ready=1
            break
        fi
        sleep 0.5
    done
    [ "$ready" = 1 ] || { podman logs "$CONTAINER" 2>&1 | tail -30 >&2; exit 2; }

    cat > "$WORK/env" <<ENV
DB_HOST=localhost
DB_PORT=$PG_PORT
DB_DATABASE=tessel
DB_USER=app
DB_PASSWORD=$password
DB_OWNER_USER=tessel_owner
DB_OWNER_PASSWORD=$owner_password
DB_SSL_ROOT_CERT=$WORK/certs/ca.crt
DB_SSL_CERT=$WORK/certs/client.crt
DB_SSL_KEY=$WORK/certs/client.key
ENV
    chmod 600 "$WORK/env"
    update_env
    echo "PostgreSQL on 127.0.0.1:$PG_PORT, mutual TLS"
    echo "DB_* lines written to apps/server/.env"
}

case "${1:-}" in
    start)
        trap 'rc=$?; if [ $rc -ne 0 ]; then stop; fi' EXIT
        start
        trap - EXIT
        ;;
    stop) stop ;;
    *) echo "usage: $0 start|stop" >&2; exit 2 ;;
esac
