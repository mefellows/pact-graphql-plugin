#!/usr/bin/env bash
#
# Checks the one version relationship that has to hold: the DSL's minimum plugin version must be
# satisfiable by the plugin this repository builds.
#
# It exists because that relationship was once broken silently. release-please bumped the npm
# package to 1.0.0 while the crate went to 0.2.0, and the DSL's floor was wired to the npm
# version, so it asked for `graphql:1.0.0`. No such plugin existed, pact never loaded it, and
# every interaction was recorded unvalidated -- surfacing as example tests failing with
# "Unexpected end of JSON input", nothing that pointed at a version.
#
# Note what is deliberately *not* enforced: the crate and the npm package are not required to
# share a version. The plugin driver treats the requested version as a floor, not a pin
# (`pact-plugins` `utils::versions_compatible`: it loads the highest installed plugin at or above
# it), so an npm 0.3.0 that declares a floor of 0.1.0 works perfectly well against a 0.2.0 plugin.
# Equality would be tidier, but release-please only bumps packages that have releasable commits,
# so a change touching only `js/` bumps the npm package alone. Requiring equality meant failing
# CI over a cosmetic difference the tool cannot avoid.
#
# Run from the repository root, or via `just check-versions`.

set -euo pipefail

cd "$(dirname "$0")/.."

cargo_version=$(grep -m1 '^version' pact-graphql-plugin/Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
npm_version=$(node -p "require('./js/pact-graphql-helper/package.json').version")
floor=$(grep -m1 "DEFAULT_PLUGIN_VERSION = " js/pact-graphql-helper/src/dsl.ts | sed -E "s/.*'(.*)'.*/\1/")
installs=$(grep -m1 "^export const PLUGIN_VERSION = " js/pact-graphql-helper/src/install.ts | sed -E "s/.*'(.*)'.*/\1/")

echo "plugin crate (Cargo.toml):   $cargo_version"
echo "npm package (package.json):  $npm_version"
echo "DSL minimum plugin version:  $floor"
echo "npm installs plugin version: $installs"
echo

if [ "$cargo_version" != "$npm_version" ]; then
  echo "note: the crate and npm package are on different versions. That is allowed -- they are"
  echo "      released independently, and compatibility is expressed by the floor below."
  echo
fi

# The floor must be satisfiable by the plugin this repository builds. A floor *above* the built
# version means nothing can ever match it, so the plugin silently never loads.
lowest=$(printf '%s\n%s\n' "$floor" "$cargo_version" | sort -V | head -1)
if [ "$lowest" != "$floor" ]; then
  echo "FAIL: the DSL requires plugin >= $floor but this repo builds $cargo_version." >&2
  echo "      Nothing would satisfy that floor, so the plugin would never load and interactions" >&2
  echo "      would be recorded unvalidated. Lower DEFAULT_PLUGIN_VERSION in" >&2
  echo "      js/pact-graphql-helper/src/dsl.ts, or release a newer plugin first." >&2
  exit 1
fi

# The installer downloads a specific plugin release. Pointing it at a version this repo has not
# reached yet means downloading an asset from a release that does not exist.
newest=$(printf '%s\n%s\n' "$installs" "$cargo_version" | sort -V | tail -1)
if [ "$newest" != "$cargo_version" ]; then
  echo "FAIL: the npm package installs plugin $installs but this repo is only at $cargo_version." >&2
  echo "      That release does not exist yet, so the download would 404. Update PLUGIN_VERSION" >&2
  echo "      in js/pact-graphql-helper/src/install.ts once the plugin release is out." >&2
  exit 1
fi

# What gets installed must itself satisfy the floor the DSL asks for.
lowest_installed=$(printf '%s\n%s\n' "$floor" "$installs" | sort -V | head -1)
if [ "$lowest_installed" != "$floor" ]; then
  echo "FAIL: the npm package installs plugin $installs, below the DSL's floor of $floor." >&2
  echo "      The plugin it installs would not satisfy the version it then asks for." >&2
  exit 1
fi

echo "OK: the DSL's plugin floor is satisfiable, and the installed version satisfies it."
