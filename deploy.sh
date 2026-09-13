#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

DEPLOY_DIR="$SCRIPT_DIR/deploy"
ARCHIVE_NAME="computeuse-x86_64.tar.gz"

echo "==> Building release binaries..."
cargo build --release --offline

echo "==> Preparing deploy directory..."
rm -rf "$DEPLOY_DIR"
mkdir -p "$DEPLOY_DIR"

cp "$SCRIPT_DIR/target/release/capture" "$DEPLOY_DIR/"
cp "$SCRIPT_DIR/target/release/interact" "$DEPLOY_DIR/"
cp "$SCRIPT_DIR/USAGE.md" "$DEPLOY_DIR/"

echo "==> Packaging into $ARCHIVE_NAME..."
tar -czf "$DEPLOY_DIR/$ARCHIVE_NAME" -C "$DEPLOY_DIR" capture interact USAGE.md

echo "==> Deployment package created successfully:"
ls -lh "$DEPLOY_DIR/$ARCHIVE_NAME"
echo ""
echo "Archive contents:"
tar -ztvf "$DEPLOY_DIR/$ARCHIVE_NAME"
rm -rf "$DEPLOY_DIR/capture"
rm -rf "$DEPLOY_DIR/interact"
rm -rf "$DEPLOY_DIR/USAGE.md"
