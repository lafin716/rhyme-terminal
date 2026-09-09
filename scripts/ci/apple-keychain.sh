#!/usr/bin/env bash
set -euo pipefail
keychain="$RUNNER_TEMP/rhyme-signing.keychain-db"
certificate="$RUNNER_TEMP/rhyme-certificate.p12"
if [[ "${1:-}" == cleanup ]]; then
  if [[ -f "$keychain" ]]; then security delete-keychain "$keychain"; fi
  rm -f "$certificate"
  exit 0
fi
if [[ -n "${APPLE_ID:-}${APPLE_PASSWORD:-}${APPLE_TEAM_ID:-}" ]]; then
  : "${APPLE_ID:?All three notarization secrets are required}"
  : "${APPLE_PASSWORD:?All three notarization secrets are required}"
  : "${APPLE_TEAM_ID:?All three notarization secrets are required}"
  : "${APPLE_CERTIFICATE:?Notarization requires a Developer ID certificate}"
fi
if [[ -z "${APPLE_CERTIFICATE:-}" ]]; then
  if [[ -n "${APPLE_SIGNING_IDENTITY:-}${APPLE_CERTIFICATE_PASSWORD:-}" ]]; then
    echo '::error::Signing identity/password configured without APPLE_CERTIFICATE'
    exit 1
  fi
  exit 0
fi
: "${APPLE_SIGNING_IDENTITY:?APPLE_SIGNING_IDENTITY is required with a certificate}"
: "${APPLE_CERTIFICATE_PASSWORD:?APPLE_CERTIFICATE_PASSWORD is required with a certificate}"
umask 077
printf '%s' "$APPLE_CERTIFICATE" | base64 --decode > "$certificate"
keychain_password="$(openssl rand -hex 24)"
security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$certificate" -P "$APPLE_CERTIFICATE_PASSWORD" -k "$keychain" -T /usr/bin/codesign -T /usr/bin/security
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$keychain_password" "$keychain" >/dev/null
# Keep the login keychain available for the rest of the runner.
security list-keychains -d user -s "$keychain" "$HOME/Library/Keychains/login.keychain-db"
rm -f "$certificate"
