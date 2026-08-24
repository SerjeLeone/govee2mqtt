#!/usr/bin/env bash
set -euo pipefail

config_file="${CONFIG_FILE:-addon/config.yaml}"
build_file="${BUILD_FILE:-addon/build.yaml}"
repository_file="${REPOSITORY_FILE:-repository.yaml}"

fail() {
  echo "::error::$*" >&2
  exit 1
}

strip_quotes() {
  local value="$1"
  value="${value%\"}"
  value="${value#\"}"
  value="${value%\'}"
  value="${value#\'}"
  printf '%s' "$value"
}

read_scalar() {
  local file="$1"
  local key="$2"
  local value

  value="$(
    awk -v key="$key" '
      index($0, key ":") == 1 {
        sub("^[^:]+:[[:space:]]*", "")
        sub("[[:space:]]+#.*$", "")
        print
        exit
      }
    ' "$file"
  )"
  strip_quotes "$value"
}

read_architectures() {
  awk '
    /^arch:[[:space:]]*$/ {
      in_arch = 1
      next
    }
    in_arch && /^[^[:space:]]/ {
      exit
    }
    in_arch && /^[[:space:]]*-[[:space:]]*/ {
      line = $0
      sub(/^[[:space:]]*-[[:space:]]*/, "", line)
      sub(/[[:space:]]+#.*$/, "", line)
      sub(/[[:space:]]+$/, "", line)
      if (line != "") {
        print line
      }
    }
  ' "$config_file"
}

read_base_image() {
  local architecture="$1"

  awk -v architecture="$architecture" '
    /^build_from:[[:space:]]*$/ {
      in_build_from = 1
      next
    }
    in_build_from && /^[^[:space:]]/ {
      exit
    }
    in_build_from {
      line = $0
      sub(/^[[:space:]]*/, "", line)
      if (index(line, architecture ":") == 1) {
        sub(/^[^:]+:[[:space:]]*/, "", line)
        sub(/[[:space:]]+#.*$/, "", line)
        gsub(/^\"|\"$/, "", line)
        gsub(/^\047|\047$/, "", line)
        print line
        exit
      }
    }
  ' "$build_file"
}

emit() {
  local key="$1"
  local value="$2"

  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf '%s=%s\n' "$key" "$value" >> "$GITHUB_OUTPUT"
  else
    printf '%s=%s\n' "$key" "$value"
  fi
}

[[ -f "$config_file" ]] || fail "Missing $config_file"
[[ -f "$build_file" ]] || fail "Missing $build_file"
[[ -f "$repository_file" ]] || fail "Missing $repository_file"

repository="${GITHUB_REPOSITORY:-}"
if [[ -z "$repository" ]]; then
  remote_url="$(git remote get-url origin 2>/dev/null || true)"
  repository="$(
    sed -E 's#^(https://github\.com/|git@github\.com:)##; s#\.git$##' \
      <<< "$remote_url"
  )"
fi

[[ "$repository" == */* ]] || fail "Cannot determine owner/repository"
repository_owner="${GITHUB_REPOSITORY_OWNER:-${repository%%/*}}"
repository_name="${repository#*/}"
registry="${REGISTRY:-ghcr.io}"

version="$(read_scalar "$config_file" version)"
addon_image="$(read_scalar "$config_file" image)"
addon_name="$(read_scalar "$config_file" name)"
addon_description="$(read_scalar "$config_file" description)"
addon_url="$(read_scalar "$config_file" url)"
repository_url="$(read_scalar "$repository_file" url)"
maintainer="$(read_scalar "$repository_file" maintainer)"

expected_url="https://github.com/$repository"
expected_addon_image="$registry/$repository_owner/{arch}-$repository_name-addon"

[[ "$version" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] \
  || fail "addon/config.yaml version must use YYYY-MM-DD: '$version'"
[[ "$addon_image" == "$expected_addon_image" ]] \
  || fail "addon image must be '$expected_addon_image', got '$addon_image'"
[[ "$addon_url" == "$expected_url" ]] \
  || fail "addon URL must be '$expected_url', got '$addon_url'"
[[ "$repository_url" == "$expected_url" ]] \
  || fail "repository URL must be '$expected_url', got '$repository_url'"
[[ "$maintainer" == "$repository_owner" ]] \
  || fail "repository maintainer must be '$repository_owner', got '$maintainer'"

if [[ -n "${RELEASE_TAG:-}" ]]; then
  [[ "$RELEASE_TAG" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] \
    || fail "Release tag must use YYYY-MM-DD: '$RELEASE_TAG'"
  [[ "$RELEASE_TAG" == "$version" ]] \
    || fail "Release tag '$RELEASE_TAG' does not match add-on version '$version'"
fi

mapfile -t architectures < <(read_architectures)
(( ${#architectures[@]} > 0 )) || fail "No architectures declared in $config_file"

architectures_json="$(jq -cn '$ARGS.positional' --args "${architectures[@]}")"
base_images='{}'
platforms='{}'
declare -A seen=()

for architecture in "${architectures[@]}"; do
  [[ -z "${seen[$architecture]:-}" ]] \
    || fail "Duplicate architecture in $config_file: $architecture"
  seen[$architecture]=1

  base_image="$(read_base_image "$architecture")"
  [[ -n "$base_image" ]] \
    || fail "No build_from image for '$architecture' in $build_file"

  platform_arch="${architecture/aarch64/arm64}"
  platform="linux/$platform_arch"

  base_images="$(
    jq -c --arg arch "$architecture" --arg image "$base_image" \
      '. + {($arch): $image}' <<< "$base_images"
  )"
  platforms="$(
    jq -c --arg arch "$architecture" --arg platform "$platform" \
      '. + {($arch): $platform}' <<< "$platforms"
  )"
done

expected_source="org.opencontainers.image.source: \"$expected_url\""
grep -Fq "$expected_source" "$build_file" \
  || fail "$build_file must declare source '$expected_url'"
grep -Fq "identity: $expected_url/.*" "$build_file" \
  || fail "$build_file must use this repository as the Cosign identity"

emit architectures "$architectures_json"
emit base_images "$base_images"
emit platforms "$platforms"
emit version "$version"
emit addon_name "$addon_name"
emit addon_description "$addon_description"
emit addon_url "$addon_url"

printf 'Validated %s %s for: %s\n' \
  "$repository" "$version" "${architectures[*]}"
