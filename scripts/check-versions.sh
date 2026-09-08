#!/usr/bin/env bash
#
# Checks that the versions which have to agree, do.
#
# Exists because they once did not: release-please bumped the npm package to 1.0.0 while the Rust
# crate went to 0.2.0, and the DSL's plugin floor was wired to the npm version. The DSL then asked
# for `graphql:1.0.0`, no such plugin was installed, and pact silently recorded every interaction
# without ever consulting the plugin. The symptom was four example tests failing with
# "Unexpected end of JSON input" -- nothing that pointed at a version mismatch.
#
# Run from the repository root, or via `just check-versions`.

set -euo pipefail

cd "$(dirname "$0")/.."

fail=0

cargo_version=$(grep -m1 '^version' pact-graphql-plugin/Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
npm_version=$(node -p "require('./js/pact-graphql-helper/package.json').version")
floor=$(grep -m1 "DEFAULT_PLUGIN_VERSION = " js/pact-graphql-helper/src/dsl.ts | sed -E "s/.*'(.*)'.*/\1/")

echo "plugin crate (Cargo.toml):      $cargo_version"
echo "npm package (package.json):     $npm_version"
echo "DSL minimum plugin version:     $floor"
echo

# 1. The crate and the npm package are released in lockstep by release-please's linked-versions
#    plugin. If they drift, that plugin has stopped grouping them -- which is exactly what
#    happened when the Rust package had no explicit `component`.
if [ "$cargo_version" != "$npm_version" ]; then
  echo "FAIL: the plugin crate ($cargo_version) and npm package ($npm_version) have diverged." >&2
  echo "      release-please's linked-versions plugin should hold them equal; check that the" >&2
  echo "      components in release-please-config.json match each package's 'component'." >&2
  fail=1
fi

# 2. The DSL's floor must be satisfiable by the plugin this repository builds. The driver loads
#    the highest installed plugin >= the floor, so a floor *above* the built version means no
#    plugin can ever match.
lowest=$(printf '%s\n%s\n' "$floor" "$cargo_version" | sort -V | head -1)
if [ "$lowest" != "$floor" ]; then
  echo "FAIL: the DSL requires plugin >= $floor but this repo builds $cargo_version." >&2
  echo "      Nothing would satisfy that floor, so the plugin would never load and interactions" >&2
  echo "      would be recorded unvalidated. Lower DEFAULT_PLUGIN_VERSION in" >&2
  echo "      js/pact-graphql-helper/src/dsl.ts, or release a newer plugin first." >&2
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "OK: versions are consistent."
fi

exit "$fail"
