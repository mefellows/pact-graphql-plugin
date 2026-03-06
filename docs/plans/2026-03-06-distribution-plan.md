# GraphQL Plugin Distribution Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `pact-plugin.json`, Justfile workflows, and local install scripts so we can build per-platform bundles matching Pact plugin release expectations.

**Architecture:** Version is sourced from `Cargo.toml`; bundling pipelines stage binaries under `dist/`, gzip them per target, and emit `.sha256` files. A Justfile orchestrates `bundle`, `bundle-all`, and `install` commands. Local install copies the binary + manifest into `~/.pact/plugins/graphql-<version>/`.

**Tech Stack:** Rust (cargo), Just, bash (jq/shasum/gzip), optional PowerShell on Windows for parity.

---

### Task 1: Add Pact plugin manifest

**Files:**
- Create: `pact-plugin.json`
- Modify: `README.md` (mention manifest)

**Step 1: Draft manifest**

```json
{
  "manifestVersion": 1,
  "name": "graphql",
  "version": "0.0.0", // placeholder updated by bundler
  "executable": "./pact-graphql-plugin",
  "description": "GraphQL Pact plugin",
  "entryPoints": [{ "type": "content-matcher", "contentTypes": ["application/json"] }]
}
```

**Step 2: Document manifest purpose**

Update `README.md` with a short “Distribution” subsection directing developers to the manifest + bundling workflow.

**Step 3: Commit**

```bash
```

### Task 2: Introduce Justfile scaffolding

**Files:**
- Create: `Justfile`
- Modify: `README.md` (usage instructions)

**Step 1: Define variables**

```make
PLUGIN_VERSION := `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="pact-graphql-plugin") | .version'`
SUPPORTED_TARGETS := x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-pc-windows-msvc aarch64-pc-windows-msvc
```

**Step 2: Add helper recipes**
- `version:` prints `PLUGIN_VERSION`
- `target-label target=` maps triples → `<os>-<arch>` strings via `case`

**Step 3: Document usage**
- Expand README distribution section to mention `just version`.

**Step 4: Commit**

```bash
```

### Task 3: Implement `just bundle`

**Files:**
- Modify: `Justfile`

**Step 1: Add `bundle target=` recipe**

Pseudo:
```make
bundle target="${HOST}":
    cargo build --release --target {{target}}
    label=`just target-label target={{target}}`
    mkdir -p dist/{{target}}
    cp target/{{target}}/release/pact-graphql-plugin{{ext}} dist/{{target}}/
    cp pact-plugin.json dist/{{target}}/pact-plugin.json
    jq ".version = \"${PLUGIN_VERSION}\"" pact-plugin.json > dist/{{target}}/pact-plugin.json
    tar -C dist/{{target}} -czf pact-graphql-plugin-${label}${extSuffix}.gz pact-graphql-plugin{{ext}}
    shasum -a 256 pact-graphql-plugin-${label}${extSuffix}.gz > pact-graphql-plugin-${label}${extSuffix}.gz.sha256
```
Handle `.exe` suffix for Windows.

**Step 2: Add `bundle-all` loop**

```make
bundle-all:
    @for target in {{SUPPORTED_TARGETS}}; do just bundle target=$$target; done
```

**Step 3: Update README**
- Add instructions on running `just bundle` and `bundle-all`.

**Step 4: Commit**

```bash
```

### Task 4: Implement local install workflow

**Files:**
- Modify: `Justfile`
- Create: `scripts/install-plugin.sh` (optional helper)

**Step 1: Helper to detect host triple**

Add `host-triple :=` command inside Just: `rustc -Vv | awk '/host/ {print $$2}'`.

**Step 2: Add `install` recipe**

Pseudo:
```make
install:
    host=`just host-triple`
    label=`just target-label target=$$host`
    archive=pact-graphql-plugin-$${label}.gz
    if [ ! -f $$archive ]; then just bundle target=$$host; fi
    tmp=$$(mktemp -d)
    cp $$archive $$tmp
    gzip -d $$tmp/$$archive
    mkdir -p ~/.pact/plugins/graphql-${PLUGIN_VERSION}
    mv $$tmp/pact-graphql-plugin${ext} ~/.pact/plugins/graphql-${PLUGIN_VERSION}/
    cp dist/$$host/pact-plugin.json ~/.pact/plugins/graphql-${PLUGIN_VERSION}/
    chmod +x ~/.pact/plugins/graphql-${PLUGIN_VERSION}/pact-graphql-plugin${ext}
```

**Step 3: README instructions**
- Document `just install` (mention requirement that plugin binary be discoverable in `~/.pact/plugins`).

**Step 4: Commit**

```bash
```

### Task 5: Smoke test bundles

**Files:**
- n/a

**Step 1: Run host bundle**

```bash
just bundle
ls pact-graphql-plugin-*.gz pact-graphql-plugin-*.gz.sha256
```
Expected: gzip + checksum created for host label.

**Step 2: Install locally**

```bash
just install
ls ~/.pact/plugins/graphql-${PLUGIN_VERSION}
```
Expected: manifest + executable present.

**Step 3: Manual verification**
- Run `~/.pact/plugins/graphql-${PLUGIN_VERSION}/pact-graphql-plugin --help` to ensure binary works.

**Step 4: Commit resulting dist cleanup (if any)**
- Ensure artifacts are gitignored (update `.gitignore` if necessary) and no new tracked files remain.

---

Plan complete and saved to `docs/plans/2026-03-06-distribution-plan.md`. Two execution options:

1. Subagent-Driven (this session) – I dispatch a fresh subagent per task via @superpowers/subagent-driven-development.
2. Parallel Session – Spin up a new session dedicated to implementation using @superpowers/executing-plans.

Which approach do you prefer?
