#!/usr/bin/env bash
set -euo pipefail

if ! command -v lefthook &> /dev/null; then
  echo "lefthook not found. Install it first:"
  echo ""
  echo "  brew install lefthook"
  echo "  # or"
  echo "  cargo install lefthook"
  echo ""
  exit 1
fi

lefthook install
echo "Lefthook hooks installed."
