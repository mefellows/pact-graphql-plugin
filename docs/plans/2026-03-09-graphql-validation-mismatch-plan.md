# GraphQL Validation Mismatch Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Surface GraphQL schema/query validation failures as structured mismatches in compare_contents and verify_interaction.

**Architecture:** Add a shared helper to turn validation errors into a single `PluginContentMismatch` entry and return it from compare/verify instead of throwing.

**Tech Stack:** Rust, pact-plugin-driver, GraphQL parser/validator

---

### Task 1: Add mismatch helper for validation errors

**Files:**
- Modify: `pact-graphql-plugin/src/graphql_payload.rs`
- Modify: `pact-graphql-plugin/src/server.rs`

**Step 1: Write failing tests**

Add a unit test in `pact-graphql-plugin/src/server.rs` that triggers invalid query validation in compare_contents and asserts a single mismatch with path `/payload/query_document` and a message containing `GraphQL query validation failed`.

**Step 2: Run test to confirm failure**

Run:
```bash
cargo test -p pact_graphql_plugin server::tests::compare_contents_invalid_query -- --exact
```
Expected: FAIL (no mismatch / wrong mismatch).

**Step 3: Implement mismatch helper**

Add a helper that converts a validation error string into a `RequestMismatch` with:

```rust
RequestMismatch {
  path: "/payload/query_document".to_string(),
  mismatch: "GraphQL query validation failed".to_string(),
  expected: Some(expected_query),
  actual: Some(actual_query),
  diff: Some(validation_error_string),
}
```

**Step 4: Use helper in compare_contents**

When canonicalization fails due to validation, return a response with a single mismatch instead of raising a Status error.

**Step 5: Run tests**

```bash
cargo test -p pact_graphql_plugin server::tests::compare_contents_invalid_query -- --exact
```
Expected: PASS.

**Step 6: Commit**

```bash
git add pact-graphql-plugin/src/graphql_payload.rs pact-graphql-plugin/src/server.rs
git commit -m "fix: surface graphql validation mismatches"
```

---

### Task 2: Mirror mismatch behavior in verify_interaction

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs`

**Step 1: Write failing verify test**

Add a unit test in `pact-graphql-plugin/src/server.rs` for `verify_interaction` with an invalid query. Assert it returns a single mismatch with the same path and message as above.

**Step 2: Run test to confirm failure**

```bash
cargo test -p pact_graphql_plugin server::tests::verify_interaction_invalid_query -- --exact
```
Expected: FAIL.

**Step 3: Implement mismatch mapping in verify_interaction**

Use the same helper to return a mismatch when validation fails.

**Step 4: Run tests**

```bash
cargo test -p pact_graphql_plugin server::tests::verify_interaction_invalid_query -- --exact
```
Expected: PASS.

**Step 5: Commit**

```bash
git add pact-graphql-plugin/src/server.rs
git commit -m "test: add verify_interaction validation mismatch"
```

---

## Verification

- `cargo test -p pact_graphql_plugin`
