# Enrich GraphQL Config Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Store canonical GraphQL request payload + inline SDL in `GraphqlPluginConfig`, validate queries against SDL, and update tests accordingly.

**Architecture:** GraphqlInteractionBuilder canonicalizes query/document/variables, Base64-encodes SDL, and validates using `graphql_parser`. The server reuses the enriched config; tests assert the new behavior.

**Tech Stack:** Rust (`graphql_parser`, `serde_json`, `base64`), pact_plugin_driver, cargo test harness.

---

### Task 1: Update data structures

**Files:** `pact-graphql-plugin/src/interaction.rs`

1. Introduce `GraphqlRequestPayload` struct with canonical fields and embed it in `GraphqlPluginConfig` alongside `schema_inline_base64`.
2. Adjust serde derives to serialize/deserialize the new structure.

### Task 2: Canonicalization helpers

**Files:** `pact-graphql-plugin/src/interaction.rs`, `pact-graphql-plugin/Cargo.toml`

1. Implement dedent logic (match JS helper), variable canonicalization via `serde_json::Value`, SDL normalization (trim + newline).
2. Add `base64` encoding plus `graphql_parser` dependency (update Cargo.toml/lock).

### Task 3: Validation integration

**Files:** `pact-graphql-plugin/src/interaction.rs`, `pact-graphql-plugin/src/server.rs`

1. Parse SDL + query using `graphql_parser`, build type/field maps, and verify operations/fields exist.
2. Update `GraphqlInteractionBuilder::build` to populate canonical payload + inline schema, returning errors on validation failure.
3. Ensure `config_to_struct` serializes the new fields.

### Task 4: Tests

**Files:** `pact-graphql-plugin/tests/interaction_tests.rs`

1. Add tests confirming canonical payload + inline schema on success.
2. Add test asserting validation failure when query references unknown field.
3. Run `cargo fmt` and `cargo test --package pact-graphql-plugin tests::interaction_tests`.

### Task 5: Commit

**Files:** n/a

1. Stage modified Rust files + Cargo manifests.
2. Commit with `feat: enrich graphql interaction config`.

---

Plan complete and saved to `docs/plans/2026-03-06-enrich-graphql-config-plan.md`. Which execution mode?
