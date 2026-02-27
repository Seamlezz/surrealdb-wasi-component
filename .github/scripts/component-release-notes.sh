#!/usr/bin/env bash
set -euo pipefail

component_id="${1:-}"
version="${2:-}"

if [[ -z "$component_id" || -z "$version" ]]; then
  echo "Usage: $0 <component-id> <version>" >&2
  exit 1
fi

case "$component_id" in
  sdk)
    title="surrealdb-component-sdk"
    tag_prefix="sdk"
    path_filter="crates/surrealdb-component-sdk"
    ;;
  host-adapter)
    title="surrealdb-host-adapter"
    tag_prefix="host-adapter"
    path_filter="crates/surrealdb-host-adapter"
    ;;
  wit)
    title="seamlezz:surrealdb WIT package"
    tag_prefix="wit"
    path_filter="wit"
    ;;
  *)
    echo "Unknown component id: $component_id" >&2
    exit 1
    ;;
esac

current_tag="${tag_prefix}-v${version}"
previous_tag="$(git tag -l "${tag_prefix}-v*" --sort=-v:refname | grep -vx "$current_tag" | head -n 1 || true)"

echo "## ${title} v${version}"
echo

if [[ -n "$previous_tag" ]]; then
  echo "Changes since \`${previous_tag}\`:"
  echo
  commits="$(git log --pretty=format:'- %s (%h)' "${previous_tag}..HEAD" -- "$path_filter" || true)"
else
  echo "Initial component release notes from repository history."
  echo
  commits="$(git log --pretty=format:'- %s (%h)' -- "$path_filter" || true)"
fi

if [[ -z "$commits" ]]; then
  echo "- Version bump release"
else
  printf '%s\n' "$commits"
fi
