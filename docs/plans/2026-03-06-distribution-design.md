# GraphQL Plugin Distribution Design

**Date:** 2026-03-06  
**Author:** OpenCode (with Matt)

## Goals
- Produce release artifacts compatible with the Pact plugin driver (matching `pact-protobuf-plugin` layout)
- Automate versioned bundles + local installation via a `just` workflow
- Keep Cargo as the source of truth for plugin versioning

## Manifest
- Add `pact-plugin.json` at repo root (checked into git). Contents include plugin name (`graphql`), description, author info, hooks, and supported content types as per https://github.com/pact-foundation/pact-plugins/blob/main/docs/plugin-driver-design.md.
- Version in the manifest is templated from `Cargo.toml` during bundling (copy file to staging and update the `version` field).

## Artifact Layout
- For each target triple (`x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` initially):
  - Compile with `cargo build --release --target <triple>`
  - Copy the resulting binary to `dist/<triple>/pact-graphql-plugin[.exe]`
  - Produce a gzip containing only the binary named `pact-graphql-plugin-<os>-<arch>[.exe].gz` (matching the pact-protobuf naming convention). Example: `pact-graphql-plugin-macos-x86_64.gz` or `pact-graphql-plugin-windows-aarch64.exe.gz`
  - Generate a `.sha256` file for each gzip using `shasum -a 256 <file> > <file>.sha256`
- Root release assets:
  - `pact-plugin.json` (one copy per release)
  - One `.gz` + `.sha256` pair per supported target
- The plugin CLI reads the manifest and downloads the correct gzip for the user’s platform, mirroring https://github.com/pactflow/pact-protobuf-plugin/releases/tag/v-0.7.0

## Justfile workflow
### Variables
- `PLUGIN_VERSION := $(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="pact-graphql-plugin") | .version')`
- `SUPPORTED_TARGETS := x86_64-apple-darwin aarch64-apple-darwin ...` (list can be extended)

### Recipes
1. `just version`
   - Outputs current version from Cargo, reused by other recipes (ensures manifest + filenames stay consistent)

2. `just bundle target=<triple>`
   - `cargo build --release --target {{target}}`
   - Determine OS/arch label (e.g., `macos-x86_64`) for naming via a helper function in the Justfile
   - Stage binary → `dist/{{target}}/pact-graphql-plugin[.exe]`
   - Copy `pact-plugin.json` into `dist/{{target}}/pact-plugin.json` and templated `version`
   - `tar`/`gzip` only the binary into `pact-graphql-plugin-<label>.gz`
   - Generate checksum file `...gz.sha256`

3. `just bundle-all`
   - Loops over `SUPPORTED_TARGETS`, invoking `just bundle target=...`

4. `just install`
   - Detects host triple from `rustc -Vv` (maps to the same label used above)
   - Ensures `pact-graphql-plugin-<label>.gz` exists (builds via `just bundle target=<host>` if not)
   - `gzip -d` → copy `pact-graphql-plugin[.exe]` and `pact-plugin.json` into `~/.pact/plugins/graphql-<version>/`
   - Makes the binary executable on Unix hosts

## Open Points / Future Work
- Cross-compilation prerequisites are on the user/CI (e.g., `rustup target add`, `xcode-select`, `zig-cc` or `cross` if needed) – document required toolchains
- Later CI (GitHub Actions) can call `just bundle-all` on macOS, Ubuntu, Windows runners to produce release artifacts and upload them
- Consider adding `.zip` artifacts for Windows if needed by users (pact CLI currently only requires `.gz` per the protobuf plugin precedent)

## Next Steps
1. Implement `pact-plugin.json` template + version substitution
2. Author the Justfile with `bundle`, `bundle-all`, `install` recipes
3. Provide documentation (README section) on how to bundle and install locally
