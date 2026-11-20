#!/usr/bin/env bash
# =============================================================================
# Pile de développement : PostgreSQL derrière BlackGate, comme en production
# (repris de Klinivia).
#
#   scripts/stack.sh start   # certificats, PostgreSQL (podman), BlackGate (hôte)
#   scripts/stack.sh stop    # arrête les deux et efface .stack/
#   scripts/stack.sh reload  # BlackGate relit columns.yaml (SIGHUP), données gardées
#
# Tessel n'atteint PostgreSQL qu'à travers BlackGate, en TLS mutuel : pas de
# raccourci en clair, en développement non plus. `start` écrit les lignes DB_*
# dans apps/server/.env (mots de passe neufs à chaque démarrage) ; les ports
# sont fixes.
#
# Deux rôles : `tessel_owner` possède le schéma et joue les migrations
# (scripts/migrate.sh) ; `app`, sous lequel l'application tourne, ne possède
# rien et reçoit ses droits des migrations.
#
# Prérequis : podman, python3, openssl, et les binaires de BlackGate
# (BLACKGATE_BIN et BLACKGATE_CLI ; par défaut la build release du dépôt blackgate).
# =============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLACKGATE_RELEASE="${BLACKGATE_RELEASE:-/mnt/depth/--=projets=--/--=github=--/c9up/blackgate/target/release}"
BLACKGATE_BIN="${BLACKGATE_BIN:-$BLACKGATE_RELEASE/blackgate}"
BLACKGATE_CLI="${BLACKGATE_CLI:-$BLACKGATE_RELEASE/blackgate-cli}"
WORK="$ROOT/.stack"
ENV_FILE="$ROOT/apps/server/.env"
CONTAINER=tessel-postgres
PG_PORT="${STACK_PG_PORT:-5461}"
PROXY_PORT="${STACK_PROXY_PORT:-5460}"
SITE=tessel_dev

stop() {
    if [ -f "$WORK/proxy.pid" ]; then kill "$(cat "$WORK/proxy.pid")" 2>/dev/null || true; fi
    podman rm -f "$CONTAINER" >/dev/null 2>&1 || true
    python3 -I -c 'import shutil, sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$WORK"
}

wait_port() { # wait_port <port> <quoi> <journal>
    for _ in $(seq 1 100); do
        if (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; then return 0; fi
        sleep 0.1
    done
    echo "$2 n'a pas démarré sur le port $1" >&2
    if [ -f "$3" ]; then tail -40 "$3" >&2; fi
    return 1
}

fingerprint() {
    python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["clients"][0]["fingerprint"])' "$WORK/certs.json"
}

# La configuration de BlackGate, avec les colonnes de deploy/blackgate/columns.yaml.
write_config() { # write_config <empreinte du client>
    local columns
    columns="$(grep -v '^\s*#' "$ROOT/deploy/blackgate/columns.yaml" | grep -v '^\s*$' | sed 's/^/    /')"
    cat > "$WORK/blackgate.yaml" <<YAML
proxy:
  listen_addr: "127.0.0.1:$PROXY_PORT"
  pool_size: 16
  backend_acquire_timeout_secs: 5
  tls_handshake_timeout_secs: 5
  shutdown_grace_period_secs: 1
postgres:
  address: "127.0.0.1"
  port: $PG_PORT
  tls:
    enabled: true
    ca_cert_path: "$WORK/certs/ca.crt"
    server_name: "localhost"
tls:
  cert_path: "$WORK/certs/server.crt"
  key_path: "$WORK/certs/server.key"
  ca_cert_path: "$WORK/certs/ca.crt"
encryption:
  master_key_path: "$WORK/certs/master.key"
  columns:
$columns
metrics:
  enabled: false
audit_log:
  output: stdout
sandbox:
  mode: single-process
  enabled: false
tenants:
  $SITE:
    cert_fingerprint: "$1"
    # Le propriétaire passe par le même client en développement seulement.
    allowed_db_users: ["app", "tessel_owner"]
YAML
    chmod 600 "$WORK/blackgate.yaml"
    "$BLACKGATE_CLI" config validate "$WORK/blackgate.yaml" >/dev/null
}

reload() {
    [ -f "$WORK/proxy.pid" ] || { echo "aucune pile en cours : scripts/stack.sh start" >&2; exit 2; }
    write_config "$(fingerprint)"
    kill -HUP "$(cat "$WORK/proxy.pid")"
    echo "BlackGate a relu sa configuration"
}

# Remplace (ou ajoute) les clés DB_* dans apps/server/.env, sans toucher au reste.
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
    [ -x "$BLACKGATE_BIN" ] || { echo "absent : $BLACKGATE_BIN (cargo build --release dans le dépôt blackgate)" >&2; exit 2; }
    [ -x "$BLACKGATE_CLI" ] || { echo "absent : $BLACKGATE_CLI" >&2; exit 2; }
    [ -d "$WORK" ] && { echo "$WORK existe : scripts/stack.sh stop d'abord" >&2; exit 2; }
    mkdir -p "$WORK/pg"
    chmod 700 "$WORK"
    local password owner_password
    password="$(openssl rand -hex 16)"
    owner_password="$(openssl rand -hex 16)"

    # AC, certificat serveur (proxy et PostgreSQL), un certificat client, clé maîtresse.
    "$BLACKGATE_CLI" -o json certs generate --dir "$WORK/certs" --client "$SITE" > "$WORK/certs.json"

    # PostgreSQL : TLS seulement sur TCP, SCRAM.
    cp "$WORK/certs/server.crt" "$WORK/certs/server.key" "$WORK/pg/"
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
hostssl   all all 0.0.0.0/0    scram-sha-256
hostnossl all all 0.0.0.0/0    reject
HBA
    podman run -d --name "$CONTAINER" -p "127.0.0.1:$PG_PORT:5432" \
        -e POSTGRES_PASSWORD="$(openssl rand -hex 16)" \
        -v "$WORK/pg:/stack:ro,Z" docker.io/library/postgres:17 \
        bash -c 'cp /stack/server.crt /stack/server.key /stack/hba.conf /tmp/ \
            && chown postgres /tmp/server.key /tmp/hba.conf && chmod 600 /tmp/server.key \
            && cp /stack/init.sql /docker-entrypoint-initdb.d/ \
            && exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/tmp/server.crt \
               -c ssl_key_file=/tmp/server.key -c hba_file=/tmp/hba.conf -c password_encryption=scram-sha-256' >/dev/null
    # L'entrypoint joue init.sql sur un serveur temporaire, puis démarre le vrai.
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

    write_config "$(fingerprint)"
    # BlackGate ne charge ses clés que sous ./certs, ./config ou ./secrets.
    (cd "$WORK" && RUST_LOG=info exec "$BLACKGATE_BIN" --config "$WORK/blackgate.yaml") > "$WORK/blackgate.log" 2>&1 &
    echo $! > "$WORK/proxy.pid"
    wait_port "$PROXY_PORT" "BlackGate" "$WORK/blackgate.log"

    cat > "$WORK/env" <<ENV
DB_HOST=localhost
DB_PORT=$PROXY_PORT
DB_DATABASE=tessel
DB_USER=app
DB_PASSWORD=$password
DB_OWNER_USER=tessel_owner
DB_OWNER_PASSWORD=$owner_password
# Tests seulement, jamais l'application : PostgreSQL lui-même, autour de BlackGate,
# pour vérifier ce que la base garde vraiment (une colonne chiffrée porte du chiffré).
DB_DIRECT_PORT=$PG_PORT
DB_SSL_ROOT_CERT=$WORK/certs/ca.crt
DB_SSL_CERT=$WORK/certs/clients/$SITE.crt
DB_SSL_KEY=$WORK/certs/clients/$SITE.key
ENV
    chmod 600 "$WORK/env"
    update_env
    echo "PostgreSQL sur 127.0.0.1:$PG_PORT, BlackGate sur 127.0.0.1:$PROXY_PORT"
    echo "Lignes DB_* écrites dans apps/server/.env"
}

case "${1:-}" in
    start)
        trap 'rc=$?; if [ $rc -ne 0 ]; then stop; fi' EXIT
        start
        trap - EXIT
        ;;
    stop) stop ;;
    reload) reload ;;
    *) echo "usage : $0 start|stop|reload" >&2; exit 2 ;;
esac
