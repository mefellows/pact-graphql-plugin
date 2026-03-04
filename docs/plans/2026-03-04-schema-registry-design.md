# Schema Registry Design

## Context
- The plugin must persist GraphQL SDL blobs outside pacts and refer to them via stable hashes so producers/consumers can share schema references without duplicating large strings.
- Current crate exposes an `encoder` module only; the registry will become another reusable helper consumed by upcoming builder/server tasks.
- Storage location must be configurable (temporary dirs during tests, plugin data dir in production).

## Goals
- Provide a `SchemaRegistry` type that stores canonical SDL documents and issues deterministic references.
- Canonicalize SDL before hashing to avoid duplicates caused by whitespace differences.
- Support retrieving inline SDL content using the previously issued reference.
- Ensure the root directory exists automatically so callers can point anywhere.

## Approach
1. **Module structure**: add `schema.rs` with `SchemaRegistry` and lightweight `SchemaRef` struct; re-export via `lib.rs` for integration tests and future modules.
2. **Canonicalization**: trim incoming SDL and append a trailing newline before hashing/writing; use `sha2::Sha256` to compute a 64-char lowercase hex digest; store as `<hash>.graphql` under root.
3. **Persistence**: `SchemaRegistry::store` writes the canonical SDL only if the file does not already exist, returning `SchemaRef { hash, encoding: "utf-8" }`.
4. **Retrieval**: `inline_schema` reads the file by hash and returns the contents; propagate filesystem errors via `anyhow`.
5. **Root initialization**: `SchemaRegistry::new<T: AsRef<Path>>(root: T)` stores a `PathBuf` and calls `create_dir_all` to ensure the directory exists before use.
6. **Testing**: add `tests/schema_tests.rs` using `tempfile::tempdir` to verify store + retrieve roundtrip and hash length; reuse canonicalization logic implicitly through assertions.

## Error Handling
- Bubble up IO errors from directory creation, file writing, and reading via `anyhow::Result`.
- Rely on canonical hash naming; collisions are effectively schema duplicates, so writing is skipped when the file already exists.

## Dependencies
- Add `sha2 = "0.10"` for hashing and `tempfile` dev-dependency for filesystem-isolated tests.

## Testing
- `cargo test schema_tests` ensures registry behavior; later tasks can build on these helpers without mocking the filesystem.
