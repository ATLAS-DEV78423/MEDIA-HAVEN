#!/usr/bin/env bash
# Live signed-request harness for MEDIA_HAVEN core.
# Reads the DPAPI-protected install token via PowerShell (same user scope),
# signs requests exactly like apps/desktop/src-tauri/src/commands/renderer.rs,
# and prints the response. Read-only unless a body is passed.
set -uo pipefail

TOKEN_FILE="${APPDATA}/MEDIA_HAVEN/mediahaven-core-api.blob"
API="127.0.0.1:8080"

get_token() {
  TOKEN_CACHE="${TOKEN_CACHE:-}"
  if [ -n "$TOKEN_CACHE" ]; then printf '%s' "$TOKEN_CACHE"; return; fi
  powershell.exe -NoProfile -Command '
    $p = Join-Path $env:APPDATA "MEDIA_HAVEN\mediahaven-core-api.blob"
    $blob = [System.IO.File]::ReadAllBytes($p)
    Add-Type -AssemblyName System.Security
    $plain = [System.Security.Cryptography.ProtectedData]::Unprotect($blob, $null, [System.Security.Cryptography.DataProtectionScope]::CurrentUser)
    ($plain | ForEach-Object { $_.ToString("x2") }) -join ""
  ' 2>/dev/null | tr -d '\r\n'
}

mh_post() {
  local path="$1"; shift
  local body="${1:-}"
  local method="POST"
  local token ts sig digest canonical
  token="$(get_token)"
  if [ -z "$token" ]; then echo "ERROR: could not read the install token" >&2; return 1; fi
  ts="$(date +%s)"
  # body digest = lowercase hex sha256 of the body
  digest="$(printf '%s' "$body" | sha256sum | cut -d' ' -f1)"
  canonical="${ts}
${method}
${path}
${digest}"
  sig="$(printf '%s' "$canonical" | openssl dgst -sha256 -hmac "$token" 2>/dev/null | sed 's/^.*= //')"
  if [ -z "$sig" ]; then echo "ERROR: signing failed" >&2; return 1; fi
  curl -sS -m 30 -X "$method" "http://${API}${path}" \
    -H "Authorization: Bearer ${token}" \
    -H "X-MediaHaven-Timestamp: ${ts}" \
    -H "X-MediaHaven-Sig: ${sig}" \
    -H "Content-Type: application/json" \
    --data-binary "$body" \
    -w "\n__STATUS__%{http_code}"
}

mh_get() {
  local path="$1"
  local method="GET"
  local token ts sig digest canonical
  token="$(get_token)"
  if [ -z "$token" ]; then echo "ERROR: could not read the install token" >&2; return 1; fi
  ts="$(date +%s)"
  digest="$(printf '%s' "" | sha256sum | cut -d' ' -f1)"
  canonical="${ts}
${method}
${path}
${digest}"
  sig="$(printf '%s' "$canonical" | openssl dgst -sha256 -hmac "$token" 2>/dev/null | sed 's/^.*= //')"
  curl -sS -m 30 -X "$method" "http://${API}${path}" \
    -H "Authorization: Bearer ${token}" \
    -H "X-MediaHaven-Timestamp: ${ts}" \
    -H "X-MediaHaven-Sig: ${sig}" \
    -w "\n__STATUS__%{http_code}"
}

# Split the HTTP status off the body for readable output.
mh_post_pretty() { mh_post "$@" | sed 's/__STATUS__/\nHTTP_STATUS: /'; }
mh_get_pretty()  { mh_get  "$@" | sed 's/__STATUS__/\nHTTP_STATUS: /'; }

case "${1:-}" in
  post) shift; mh_post_pretty "$@" ;;
  get)  shift; mh_get_pretty  "$@" ;;
  token) get_token; echo ;;
  *) echo "usage: $0 {post|get} <path> [json-body] | token" >&2; exit 2 ;;
esac
