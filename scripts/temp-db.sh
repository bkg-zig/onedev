#!/usr/bin/env bash
#
# Wegwerf-Postgres (pgrust) für Tests — berührt die echte DB nicht.
#
# Usage:
#   scripts/temp-db.sh start   # baut ggf. das db-Image und startet Temp-DB
#   scripts/temp-db.sh url     # gibt die Connection-URL aus
#   scripts/temp-db.sh stop    # hält an und löscht Container + Daten
#   scripts/temp-db.sh psql    # psql-Shell in der Temp-DB
#
# Port 55432 (nicht 5432), damit es keinen Konflikt mit der echten DB gibt.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTAINER="onedev-temp-db"
PORT="${ONEDEV_TEMP_DB_PORT:-55432}"
IMAGE="${ONEDEV_TEMP_DB_IMAGE:-bkg-core-admin-db:mango}"
PASSWORD="onedev-temp"

url() {
    echo "postgres://postgres:${PASSWORD}@127.0.0.1:${PORT}/onedev"
}

cmd_start() {
    if docker ps --format '{{.Names}}' | grep -qx "$CONTAINER"; then
        echo "Temp-DB läuft bereits: $(url)"
        return 0
    fi
    if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
        echo "Baue DB-Image $IMAGE ..."
        "$ROOT/scripts/docker-build.sh" db
    fi
    docker run -d --rm --name "$CONTAINER" \
        -p "127.0.0.1:${PORT}:5432" \
        -e POSTGRES_USER=postgres \
        -e POSTGRES_PASSWORD="$PASSWORD" \
        -e POSTGRES_DB=onedev \
        "$IMAGE" >/dev/null
    echo -n "Warte auf Postgres"
    for _ in $(seq 1 30); do
        if docker exec "$CONTAINER" pg_isready -U postgres -d onedev >/dev/null 2>&1; then
            echo
            echo "Temp-DB bereit: $(url)"
            return 0
        fi
        echo -n "."
        sleep 1
    done
    echo
    echo "FEHLER: Temp-DB wurde nicht bereit" >&2
    return 1
}

cmd_stop() {
    docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
    echo "Temp-DB gestoppt, Daten verworfen."
}

case "${1:-}" in
    start) cmd_start ;;
    stop) cmd_stop ;;
    url) url ;;
    psql)
        exec docker exec -it "$CONTAINER" psql -U postgres -d onedev
        ;;
    -h|--help|"")
        sed -n '2,/^$/p' "$0"
        ;;
    *)
        echo "Unbekannt: $1 (start|stop|url|psql)" >&2
        exit 1
        ;;
esac
