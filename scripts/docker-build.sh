#!/usr/bin/env bash
#
# Ein Befehl für den kompletten bkg-core-admin OneDev Stack.
#
# Usage:
#   scripts/docker-build.sh build [service...]  # Images bauen (Standard: db core web)
#   scripts/docker-build.sh up [service...]    # bauen + starten (Standard: db core web)
#   scripts/docker-build.sh up-all             # alles inkl. gateway-Profil
#   scripts/docker-build.sh down               # stoppen + Container entfernen
#   scripts/docker-build.sh stop               # nur anhalten
#   scripts/docker-build.sh start              # angehaltene starten
#   scripts/docker-build.sh restart [service...] # neu starten
#   scripts/docker-build.sh update             # Images neu bauen + Stack neu starten
#   scripts/docker-build.sh logs [service...]  # Logs folgen (Standard: alle)
#   scripts/docker-build.sh ps                 # Status
#   scripts/docker-build.sh config             # Compose-Config validieren
#
# Muss von nirgendwo Besonderem laufen; das Skript pinnt das Repo-Root als
# Projektverzeichnis, damit relative Build-Kontexte immer auflösen (siehe
# docs/compose-web-test.md).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

COMPOSE=(docker compose --project-directory . -f docker/compose.yml)
DEFAULT_SERVICES=(db core web)

usage() {
    sed -n '2,/^$/p' "$0" | grep '^#'
}

cmd_build() {
    if [ $# -eq 0 ]; then
        "${COMPOSE[@]}" build "${DEFAULT_SERVICES[@]}"
    else
        "${COMPOSE[@]}" build "$@"
    fi
}

cmd_up() {
    if [ $# -eq 0 ]; then
        "${COMPOSE[@]}" up -d --build "${DEFAULT_SERVICES[@]}"
    else
        "${COMPOSE[@]}" up -d --build "$@"
    fi
}

case "${1:-}" in
    build)
        shift
        cmd_build "$@"
        ;;
    up)
        shift
        cmd_up "$@"
        ;;
    up-all)
        "${COMPOSE[@]}" --profile gateway up -d --build
        ;;
    down)
        "${COMPOSE[@]}" --profile gateway down "${@:2}"
        ;;
    stop)
        shift
        if [ $# -eq 0 ]; then
            "${COMPOSE[@]}" --profile gateway stop
        else
            "${COMPOSE[@]}" stop "$@"
        fi
        ;;
    start)
        shift
        if [ $# -eq 0 ]; then
            "${COMPOSE[@]}" --profile gateway start
        else
            "${COMPOSE[@]}" start "$@"
        fi
        ;;
    restart)
        shift
        if [ $# -eq 0 ]; then
            "${COMPOSE[@]}" --profile gateway restart
        else
            "${COMPOSE[@]}" restart "$@"
        fi
        ;;
    update)
        # Images frisch bauen, Container neu erstellen, alte Images aufräumen.
        "${COMPOSE[@]}" --profile gateway up -d --build
        docker image prune -f >/dev/null
        ;;
    logs)
        shift
        if [ $# -eq 0 ]; then
            "${COMPOSE[@]}" --profile gateway logs -f
        else
            "${COMPOSE[@]}" logs -f "$@"
        fi
        ;;
    ps|status)
        "${COMPOSE[@]}" --profile gateway ps
        ;;
    config)
        "${COMPOSE[@]}" config --quiet && echo "Compose-Config OK"
        ;;
    -h|--help|"")
        usage
        ;;
    *)
        echo "Unbekannt: $1 — siehe --help" >&2
        exit 1
        ;;
esac

echo "OK: $1"
