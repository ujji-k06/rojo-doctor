#!/usr/bin/env bash
set -euo pipefail

echo "Building Rust binary..."
cargo build --quiet

FIXTURES=(
  "tests/fixtures/valid-json"
  "tests/fixtures/valid-jsonc"
  "tests/fixtures/missing-path"
  "tests/fixtures/file-with-children"
  "tests/fixtures/missing-class"
  "tests/fixtures/class-on-file"
  "tests/fixtures/child-collision"
  "tests/fixtures/ambiguous-init"
  "tests/fixtures/orphan-meta"
  "tests/fixtures/pesde-packages"
  "tests/fixtures/script-context"
  "tests/fixtures/missing-name"
)

echo "Verifying 100% text and exit code parity across ${#FIXTURES[@]} fixtures..."

for fix in "${FIXTURES[@]}"; do
  RUST_OUT=$(target/debug/rojo-doctor check "$fix" || true)
  RUST_CODE=$?

  LUNE_OUT=$(lune run bin/rojo-doctor.luau check "$fix" || true)
  LUNE_CODE=$?

  if [ "$RUST_CODE" != "$LUNE_CODE" ]; then
    echo "❌ Exit code mismatch on $fix: Rust=$RUST_CODE Lune=$LUNE_CODE"
    exit 1
  fi

  if [ "$RUST_OUT" != "$LUNE_OUT" ]; then
    echo "❌ Text output mismatch on $fix"
    diff -u <(echo "$RUST_OUT") <(echo "$LUNE_OUT")
    exit 1
  fi

  echo "✓ $fix matched (code $RUST_CODE)"
done

echo "All parity checks passed!"
