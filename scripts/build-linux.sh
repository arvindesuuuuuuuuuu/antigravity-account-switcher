#!/usr/bin/env bash
set -e

echo "=== Building Antigravity Account Switcher for Linux ==="
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] Installing NPM dependencies..."
npm install

echo "[2/3] Building Web Assets..."
npm run build

echo "[3/3] Compiling Tauri 2 Native Linux Binary..."
npx tauri build --no-bundle

mkdir -p bin/linux
cp src-tauri/target/release/antigravity-account-switcher bin/linux/
chmod +x bin/linux/antigravity-account-switcher

echo "=== Build Complete! ==="
echo "Binary available at: bin/linux/antigravity-account-switcher"
