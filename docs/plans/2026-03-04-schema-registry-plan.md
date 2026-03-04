# Schema Registry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a filesystem-backed schema registry so the plugin can persist canonical GraphQL SDL bodies and rehydrate them later.

**Architecture:** Introduce a `schema` module exposing `SchemaRegistry`/`SchemaRef`, canonicalizing SDL content before hashing with SHA-256. Store each schema as `<hash>.graphql` beneath a configurable root directory and provide helper to read by hash. Integration tests exercise real FS via `tempfile`.

**Tech Stack:** Rust (`anyhow`, `sha2`, `tempfile`, std::fs`, `PathBuf`), Cargo workspace tooling.

---

### Task 1: Add Dependencies

**Files:**
- Modify: `pact-graphql-plugin/Cargo.toml`

**Step 1: Declare runtime + dev dependencies**

Add `sha2 = "0.10"` under `[dependencies]` and `tempfile = "3"` under `[dev-dependencies]`.

**Step 2: Format manifest**

Run: `cargo fmt --all` (ensures `cargo fmt` available, even though manifest unaffected).

### Task 2: Write Failing Integration Test

**Files:**
- Create: `pact-graphql-plugin/tests/schema_tests.rs`

**Step 1: Implement test**

```rust
use pact_graphql_plugin::schema::{SchemaRegistry, SchemaRef};

#[test]
fn stores_and_retrieves_schema() {
    let dir = tempfile::tempdir().unwrap();
    let registry = SchemaRegistry::new(dir.path()).unwrap();
    let reference = registry.store("type Query { ping: String }").unwrap();
    assert_eq!(reference.hash.len(), 64);
    assert_eq!(reference.encoding, "utf-8");
    let inline = registry.inline_schema(&reference.hash).unwrap();
    assert!(inline.contains("type Query"));
}
```

**Step 2: Run test to confirm failure**

Run: `cargo test schema_tests`
Expected: FAIL - module `schema` not found / function missing.

### Task 3: Implement Schema Registry

**Files:**
- Create: `pact-graphql-plugin/src/schema.rs`
- Modify: `pact-graphql-plugin/src/lib.rs`

**Step 1: Define structs + constructor**

```rust
use std::path::{Path, PathBuf};
use std::fs;
use sha2::{Digest, Sha256};

pub struct SchemaRef {
    pub hash: String,
    pub encoding: String,
}

pub struct SchemaRegistry {
    root: PathBuf,
}

impl SchemaRegistry {
    pub fn new<P: AsRef<Path>>(root: P) -> anyhow::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
    // ...
}
```

**Step 2: Implement `store`**

```rust
    pub fn store(&self, sdl: &str) -> anyhow::Result<SchemaRef> {
        let canonical = format!("{}\n", sdl.trim());
        let hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        let path = self.root.join(format!("{}.graphql", hash));
        if !path.exists() {
            fs::write(&path, canonical)?;
        }
        Ok(SchemaRef { hash, encoding: "utf-8".into() })
    }
```

**Step 3: Implement `inline_schema`**

```rust
    pub fn inline_schema(&self, hash: &str) -> anyhow::Result<String> {
        let path = self.root.join(format!("{}.graphql", hash));
        Ok(fs::read_to_string(path)?)
    }
```

**Step 4: Expose module**

Add `pub mod schema;` to `src/lib.rs` alongside existing modules.

### Task 4: Format + Test + Commit

**Step 1: Format code**

Run: `cargo fmt`

**Step 2: Execute targeted tests**

Run: `cargo test schema_tests`
Expected: PASS.

**Step 3: Commit**

Run:

```bash
git add docs/plans/2026-03-04-schema-registry-design.md \
        docs/plans/2026-03-04-schema-registry-plan.md \
        pact-graphql-plugin/Cargo.toml \
        pact-graphql-plugin/src/lib.rs \
        pact-graphql-plugin/src/schema.rs \
        pact-graphql-plugin/tests/schema_tests.rs
git commit -m "feat: add schema registry"
```
