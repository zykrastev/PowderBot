#!/usr/bin/env bash
# Run from anywhere. Optional arguments are passed to espflash (e.g. --port).
set -euo pipefail

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    echo "Usage: ./flash.sh [espflash options]"
    echo "Runs core tests, builds normal firmware, and flashes without a monitor."
    echo "Example: ./flash.sh --port /dev/ttyUSB0"
    exit 0
fi

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Load the toolchain environment when installed at the usual location.
if [[ -f "$HOME/export-esp.sh" ]]; then
    source "$HOME/export-esp.sh"
fi

echo "Running core tests..."
cd "$project_dir/core"
cargo +stable test

echo "Building firmware..."
cd "$project_dir/firmware"
cargo build --target-dir "$project_dir/firmware/target"

echo "Uploading firmware..."
espflash flash --partition-table partitions.csv --flash-size 4mb \
    "$project_dir/firmware/target/xtensa-esp32-espidf/debug/firmware" "$@"

echo "Upload complete. Dashboard: http://192.168.71.1/"
