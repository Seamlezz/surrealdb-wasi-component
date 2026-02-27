#!/usr/bin/env bash
set -euo pipefail

before_sha="${BEFORE_SHA:-}"
head_sha="${HEAD_SHA:-HEAD}"
force_component="${FORCE_COMPONENT:-}"

if [[ -z "$before_sha" || "$before_sha" =~ ^0+$ ]]; then
  before_sha="$(git rev-parse "${head_sha}^" 2>/dev/null || true)"
fi

if [[ -z "$before_sha" ]]; then
  before_sha="$head_sha"
fi

semver_regex='^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$'

read_cargo_version_from_ref() {
  local ref="$1"
  local path="$2"
  git show "$ref:$path" 2>/dev/null | sed -n 's/^version = "\([^"]*\)"/\1/p' | head -n 1
}

read_wit_version_from_ref() {
  local ref="$1"
  local path="$2"
  git show "$ref:$path" 2>/dev/null | sed -n 's/^package seamlezz:surrealdb@\([0-9A-Za-z.+-]*\);/\1/p' | head -n 1
}

crate_published_version() {
  local crate_name="$1"
  local response
  response="$(curl -fsSL "https://crates.io/api/v1/crates/${crate_name}" 2>/dev/null || true)"
  if [[ -z "$response" ]]; then
    echo ""
    return
  fi
  printf '%s' "$response" | jq -r '.crate.max_stable_version // .crate.newest_version // ""'
}

append_release() {
  local id="$1"
  local version="$2"
  releases+=("$(jq -nc --arg id "$id" --arg version "$version" '{id:$id, version:$version}')")
}

releases=()

if [[ -n "$force_component" ]]; then
  case "$force_component" in
    sdk)
      sdk_now="$(read_cargo_version_from_ref "$head_sha" "crates/surrealdb-component-sdk/Cargo.toml")"
      if [[ -n "$sdk_now" && "$sdk_now" =~ $semver_regex ]]; then
        sdk_published="$(crate_published_version "surrealdb-component-sdk")"
        if [[ "$sdk_now" != "$sdk_published" ]]; then
          append_release "sdk" "$sdk_now"
        fi
      fi
      ;;
    host-adapter)
      host_now="$(read_cargo_version_from_ref "$head_sha" "crates/surrealdb-host-adapter/Cargo.toml")"
      if [[ -n "$host_now" && "$host_now" =~ $semver_regex ]]; then
        host_published="$(crate_published_version "surrealdb-host-adapter")"
        if [[ "$host_now" != "$host_published" ]]; then
          append_release "host-adapter" "$host_now"
        fi
      fi
      ;;
    wit)
      wit_now="$(read_wit_version_from_ref "$head_sha" "wit/world.wit")"
      if [[ -n "$wit_now" && "$wit_now" =~ $semver_regex ]]; then
        if ! git rev-parse -q --verify "refs/tags/wit-v${wit_now}" >/dev/null; then
          append_release "wit" "$wit_now"
        fi
      fi
      ;;
    *)
      echo "Unknown FORCE_COMPONENT value: $force_component" >&2
      exit 1
      ;;
  esac
fi

if [[ -n "$force_component" ]]; then
  if [[ ${#releases[@]} -eq 0 ]]; then
    matrix='[]'
  else
    matrix="$(printf '%s\n' "${releases[@]}" | jq -cs '.')"
  fi

  count="$(printf '%s' "$matrix" | jq 'length')"

  echo "matrix=$matrix" >> "$GITHUB_OUTPUT"
  echo "count=$count" >> "$GITHUB_OUTPUT"
  echo "Release matrix: $matrix"
  exit 0
fi

sdk_prev="$(read_cargo_version_from_ref "$before_sha" "crates/surrealdb-component-sdk/Cargo.toml")"
sdk_now="$(read_cargo_version_from_ref "$head_sha" "crates/surrealdb-component-sdk/Cargo.toml")"
if [[ -n "$sdk_now" && "$sdk_now" != "$sdk_prev" ]]; then
  if [[ "$sdk_now" =~ $semver_regex ]]; then
    sdk_published="$(crate_published_version "surrealdb-component-sdk")"
    if [[ "$sdk_now" != "$sdk_published" ]]; then
      append_release "sdk" "$sdk_now"
    fi
  fi
fi

host_prev="$(read_cargo_version_from_ref "$before_sha" "crates/surrealdb-host-adapter/Cargo.toml")"
host_now="$(read_cargo_version_from_ref "$head_sha" "crates/surrealdb-host-adapter/Cargo.toml")"
if [[ -n "$host_now" && "$host_now" != "$host_prev" ]]; then
  if [[ "$host_now" =~ $semver_regex ]]; then
    host_published="$(crate_published_version "surrealdb-host-adapter")"
    if [[ "$host_now" != "$host_published" ]]; then
      append_release "host-adapter" "$host_now"
    fi
  fi
fi

wit_prev="$(read_wit_version_from_ref "$before_sha" "wit/world.wit")"
wit_now="$(read_wit_version_from_ref "$head_sha" "wit/world.wit")"
if [[ -n "$wit_now" && "$wit_now" != "$wit_prev" ]]; then
  if [[ "$wit_now" =~ $semver_regex ]]; then
    if ! git rev-parse -q --verify "refs/tags/wit-v${wit_now}" >/dev/null; then
      append_release "wit" "$wit_now"
    fi
  fi
fi

if [[ ${#releases[@]} -eq 0 ]]; then
  matrix='[]'
else
  matrix="$(printf '%s\n' "${releases[@]}" | jq -cs '.')"
fi

count="$(printf '%s' "$matrix" | jq 'length')"

echo "matrix=$matrix" >> "$GITHUB_OUTPUT"
echo "count=$count" >> "$GITHUB_OUTPUT"

echo "Release matrix: $matrix"
