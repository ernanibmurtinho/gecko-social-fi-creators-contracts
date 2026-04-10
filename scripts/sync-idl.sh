#!/usr/bin/env bash
# sync-idl.sh — Build the program and distribute the IDL + TypeScript types
# to the API and frontend repos.
#
# Usage:
#   ./scripts/sync-idl.sh            # build for localnet (testing feature ON)
#   ./scripts/sync-idl.sh --devnet   # build for devnet (testing feature OFF)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACTS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
API_DIR="$CONTRACTS_DIR/../gecko-social-fi-creators-api"
APP_DIR="$CONTRACTS_DIR/../gecko-social-fi-creators-app"

IDL_SRC="$CONTRACTS_DIR/target/idl/gecko_vault.json"
TYPES_SRC="$CONTRACTS_DIR/target/types/gecko_vault.ts"

API_IDL_DIR="$API_DIR/src/idl"
APP_IDL_DIR="$APP_DIR/src/idl"

# ---------------------------------------------------------------------------
# Parse args
# ---------------------------------------------------------------------------

DEVNET=false
for arg in "$@"; do
  case $arg in
    --devnet) DEVNET=true ;;
    *) echo "Unknown argument: $arg" && exit 1 ;;
  esac
done

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

echo "🔨 Building gecko-vault..."

cd "$CONTRACTS_DIR"

if [ "$DEVNET" = true ]; then
  echo "   Mode: devnet (testing feature OFF)"
  anchor build
else
  echo "   Mode: localnet (testing feature ON — MIN_CLIFF_SECONDS = 1s)"
  anchor build -- --features testing
fi

echo "   ✓ Build complete"

# ---------------------------------------------------------------------------
# Verify IDL exists
# ---------------------------------------------------------------------------

if [ ! -f "$IDL_SRC" ]; then
  echo "❌ IDL not found at $IDL_SRC — build may have failed"
  exit 1
fi

if [ ! -f "$TYPES_SRC" ]; then
  echo "❌ Types not found at $TYPES_SRC — build may have failed"
  exit 1
fi

# ---------------------------------------------------------------------------
# Sync to API repo
# ---------------------------------------------------------------------------

if [ -d "$API_DIR" ]; then
  mkdir -p "$API_IDL_DIR"
  cp "$IDL_SRC"   "$API_IDL_DIR/gecko_vault.json"
  cp "$TYPES_SRC" "$API_IDL_DIR/gecko_vault.ts"
  echo "📦 API ← $API_IDL_DIR"
else
  echo "⚠️  API repo not found at $API_DIR — skipping"
fi

# ---------------------------------------------------------------------------
# Sync to App repo
# ---------------------------------------------------------------------------

if [ -d "$APP_DIR" ]; then
  mkdir -p "$APP_IDL_DIR"
  cp "$IDL_SRC"   "$APP_IDL_DIR/gecko_vault.json"
  cp "$TYPES_SRC" "$APP_IDL_DIR/gecko_vault.ts"
  echo "📦 App ← $APP_IDL_DIR"
else
  echo "⚠️  App repo not found at $APP_DIR — skipping"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

PROGRAM_ID=$(grep '"address"' "$IDL_SRC" | head -1 | awk -F'"' '{print $4}')

echo ""
echo "✅ IDL sync complete"
echo "   Program ID : $PROGRAM_ID"
echo "   IDL        : gecko_vault.json"
echo "   Types      : gecko_vault.ts"
echo "   Synced to  : api/src/idl/  app/src/idl/"
if [ "$DEVNET" = false ]; then
  echo ""
  echo "   ⚠️  Built with 'testing' feature (MIN_CLIFF_SECONDS = 1s)"
  echo "   Run './scripts/sync-idl.sh --devnet' before deploying to devnet."
fi
