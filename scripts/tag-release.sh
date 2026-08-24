#!/bin/sh
set -eu

# Prepare a release commit and annotated tag. Publishing the GitHub Release is
# intentionally a separate manual action; that event starts verification,
# multi-architecture builds, signing, and publication.
TAG_NAME=${TAG_NAME:-$(date +%Y-%m-%d)}

case "$TAG_NAME" in
  [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]) ;;
  *)
    echo "TAG_NAME must use YYYY-MM-DD: $TAG_NAME" >&2
    exit 1
    ;;
esac

if [ -n "$(git status --porcelain)" ]; then
  echo "The working tree must be clean before preparing a release" >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/$TAG_NAME" >/dev/null; then
  echo "Tag already exists: $TAG_NAME" >&2
  exit 1
fi

TAG_NAME="$TAG_NAME" ./scripts/apply-tag.sh

docker run --rm -t \
  -v "$(pwd)":/app/ \
  "ghcr.io/orhun/git-cliff/git-cliff:${GIT_CLIFF_TAG:-latest}" \
  --tag "$TAG_NAME" \
  -o addon/CHANGELOG.md \
  -c scripts/cliff.toml

git add addon/config.yaml addon/CHANGELOG.md
git commit -m "Release $TAG_NAME"
git tag -a "$TAG_NAME" -m "Release $TAG_NAME"

echo "Prepared release $TAG_NAME. Push main, then publish a GitHub Release for this tag."
