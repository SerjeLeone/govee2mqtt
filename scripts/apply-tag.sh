#!/bin/sh
set -eu

# Release tags in this fork use the same YYYY-MM-DD format as its first two
# releases. TAG_NAME may be supplied explicitly for reproducible preparation.
TAG_NAME=${TAG_NAME:-$(date +%Y-%m-%d)}

case "$TAG_NAME" in
  [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]) ;;
  *)
    echo "TAG_NAME must use YYYY-MM-DD: $TAG_NAME" >&2
    exit 1
    ;;
esac

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
sed "s/^version:.*/version: \"$TAG_NAME\"/" addon/config.yaml > "$tmp"
mv "$tmp" addon/config.yaml
chmod 644 addon/config.yaml
trap - EXIT
