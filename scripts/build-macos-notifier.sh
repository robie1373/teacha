#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Teacha Notifier"
OUT_DIR="${1:-./macos}"
SCRIPT_PATH="./scripts/macos-notifier.applescript"

mkdir -p "$OUT_DIR"
osacompile -o "$OUT_DIR/$APP_NAME.app" "$SCRIPT_PATH"

echo "Built $OUT_DIR/$APP_NAME.app"
