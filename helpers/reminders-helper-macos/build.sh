#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "Building reminders-helper..."
swiftc -o reminders-helper Sources/main.swift \
    -framework EventKit \
    -framework Foundation \
    -O

echo "✓ Built: $(pwd)/reminders-helper"
