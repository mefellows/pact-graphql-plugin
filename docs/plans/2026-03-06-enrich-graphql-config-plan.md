# Enrich GraphQL Config Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Store canonical GraphQL request payload + inline SDL in `GraphqlPluginConfig`, validate queries against SDL, and update tests accordingly.

**Architecture:** `GraphqlInteractionBuilder` canonicalizes query/document/variables, Base64-encodes SDL, and validates using `graphql_parser`. The server reuses the enriched config; tests assert the new behavior.

**Tech Stack:** Rust (`graphql_parser`, `serde_json`, `base64`), cargo test harness.

---

### Task 1: Update data structures

**Files:**
- Modify: `pact-graphql-plugin/src/interaction.rs`

**Steps:**
1. Introduce `GraphqlRequestPayload` struct (query_document, operation_name, variables_json, transport) and embed it in `GraphqlPluginConfig` along with `schema_inline_base64`.
2. Update serde derives to include the new fields (ensure backwards-compatible serialization if needed).
3. Commit `chore: expand graphql config model` if you prefer to checkpoint (optional).

### Task 2: Canonicalization helpers

**Files:**
- Modify: `pact-graphql-plugin/src/interaction.rs`
- Modify: `pact-graphql-plugin/Cargo.toml` (add `graphql_parser`)

**Steps:**
1. Implement functions for dedenting query (line-based approach), canonicalizing variables (`serde_json::Value` round-trip), and normalizing SDL (trim + trailing newline).
2. Add Base64 encoding via `base64::engine::general_purpose::STANDARD`.
3. Update `Cargo.toml`/`Cargo.lock` with `graphql_parser` dependency and run `cargo fetch` if needed.

### Task 3: Validation integration

**Files:**
- Modify: `pact-graphql-plugin/src/interaction.rs`
- Modify: `pact-graphql-plugin/src/server.rs`

**Steps:**
1. Use `graphql_parser` to parse SDL and query; build type map to validate selected fields/operations.
2. Extend `GraphqlInteractionBuilder::build` to run validation and populate `GraphqlPluginConfig` with canonical payload + inline schema. Return `anyhow::Error` on validation failure.
3. Update `config_to_struct` and related serialization helpers to include the new fields.

### Task 4: Tests

**Files:**
- Modify: `pact-graphql-plugin/tests/interaction_tests.rs`

**Steps:**
1. Add tests verifying canonical query/variables + Base64 schema stored on success.
2. Add failure test where query references unknown field, expect builder to return Err containing helpful message.
3. Run `cargo fmt` and `cargo test --package pact-graphql-plugin tests::interaction_tests`.

### Task 5: Final commit

**Files:**
- n/a

**Steps:**
1. Stage all modified files (`interaction.rs`, `server.rs`, tests, Cargo.toml/lock`).
2. Commit with `feat: enrich graphql interaction config`.

---

Plan complete and saved to `docs/plans/2026-03-06-enrich-graphql-config-plan.md`. Which execution mode?
