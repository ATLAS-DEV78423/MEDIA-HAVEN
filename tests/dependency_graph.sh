#!/usr/bin/env bash
# H3 (ADR-0005): enforce the crate layering at the Cargo.toml boundary.
#
#   * app-core and persistence are pure domain/storage: no HTTP server, no
#     container-runtime client and no Windows-API dependency.
#   * the Tauri host owns the window/signing surface: no SQL dependency.
#
# Exits 1 with an explicit message naming the offending crate and dependency
# the moment one of these rules is broken. Runnable from any cwd.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fail=0

# A dependency declaration starts (after whitespace) with the crate name,
# followed by `=` or `.workspace`. Comments (`# ...`) never match.
assert_no_deps() {
  local rel="$1" file="$2"
  shift 2
  local name
  if [ ! -f "$file" ]; then
    echo "FAIL: $rel not found" >&2
    fail=1
    return
  fi
  for name in "$@"; do
    if grep -Eq "^[[:space:]]*${name}[[:space:]]*(=|\\.)" "$file"; then
      echo "FAIL: $rel declares forbidden dependency '$name'" >&2
      fail=1
    fi
  done
}

# HTTP server stacks, container-runtime clients, Windows-API bindings.
DOMAIN_FORBIDDEN=(
  axum hyper actix-web rocket warp tide poem salvo viz tower-http tiny_http
  bollard docker docker-api podman kube kubernetes containers-api
  windows windows-sys winapi winreg
)

# SQL clients/libraries — forbidden in the host, expected only in persistence.
HOST_FORBIDDEN=(
  sqlx diesel sea-orm rusqlite sqlite libsqlite3-sys
  tokio-postgres postgres mysql mysql_async mysqlclient-sys
)

assert_no_deps "crates/app-core/Cargo.toml" \
  "$ROOT/crates/app-core/Cargo.toml" "${DOMAIN_FORBIDDEN[@]}"
assert_no_deps "crates/persistence/Cargo.toml" \
  "$ROOT/crates/persistence/Cargo.toml" "${DOMAIN_FORBIDDEN[@]}"
assert_no_deps "apps/desktop/src-tauri/Cargo.toml" \
  "$ROOT/apps/desktop/src-tauri/Cargo.toml" "${HOST_FORBIDDEN[@]}"

if [ "$fail" -ne 0 ]; then
  echo "dependency-graph: FAILED — a layer boundary was violated" >&2
  exit 1
fi

echo "dependency-graph: OK — app-core/persistence free of HTTP, container and Windows deps; host free of SQL deps"
exit 0
