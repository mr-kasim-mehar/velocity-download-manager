#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rm -f "$ROOT/src-tauri/browser-extension.zip"
cd "$ROOT/browser-extension"
zip -qr "$ROOT/src-tauri/browser-extension.zip" .
echo "Created $ROOT/src-tauri/browser-extension.zip"
