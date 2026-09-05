#!/usr/bin/env bash
# Build and exercise the exact asset uploaded by the release workflow.
set -euo pipefail

case "${1:-}" in
  amd64) target=x86_64-unknown-linux-gnu ;;
  arm64) target=aarch64-unknown-linux-gnu ;;
  *) echo "Usage: $0 {amd64|arm64}" >&2; exit 2 ;;
esac

cd "$(dirname "$0")/.."
asset="gh-assigned-linux-$1"
platform="linux/$1"

# Keep bookworm explicit: moving to a newer distro raises the glibc baseline.
docker run --rm --platform "$platform" \
  -v "$PWD:/work" -w /work \
  rust:1.90.0-bookworm \
  sh -ec '
    cargo build --release --locked --target "$1"
    cp "target/$1/release/gh-assigned" "$2"
  ' sh "$target" "$asset"

# A compiler image can hide missing runtime libraries. Test the packaged file
# in the minimum supported userspace, without installing any extra packages.
docker run --rm --platform "$platform" \
  -v "$PWD:/work:ro" -w /work \
  debian:bookworm-slim \
  sh -ec '
    test "$(getconf GNU_LIBC_VERSION)" = "glibc 2.36"
    ldd "./$1"
    "./$1" --help > /tmp/help.txt
    cmp HELP.txt /tmp/help.txt
    "./$1" -h > /tmp/short-help.txt
    cmp HELP.txt /tmp/short-help.txt
  ' sh "$asset"
