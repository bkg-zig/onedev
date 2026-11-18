#!/usr/bin/env bash
#
# Build the bkg-core-admin OneDev stack.
#
# Usage:
#   scripts/docker-build.sh [service...]   # build images (default: db core web)
#   scripts/docker-build.sh --up           # build and start db + core + web
#   scripts/docker-build.sh --up-all       # build and start everything incl. gateway
#
# Must run from anywhere; the script pins the repo root as project
# directory so relative build contexts always resolve (see
# docs/compose-web-test.md).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

UP=""
SERVICES=()
for arg in "$@"; do
    case "$arg" in
        --up) UP="db core web" ;;
        --up-all) UP="--profile gateway" ;;
        -h|--help)
            sed -n '2,/^$/p' "$0"
            exit 0
            ;;
        *) SERVICES+=("$arg") ;;
    esac
done

if [ ${#SERVICES[@]} -eq 0 ] && [ -z "$UP" ]; then
    SERVICES=(db core web)
fi

COMPOSE=(docker compose --project-directory . -f docker/compose.yml)

if [ -n "$UP" ]; then
    # shellcheck disable=SC2086
    "${COMPOSE[@]}" up -d --build $UP
else
    "${COMPOSE[@]}" build "${SERVICES[@]}"
fi

echo "OK: ${UP:-${SERVICES[*]}}"
