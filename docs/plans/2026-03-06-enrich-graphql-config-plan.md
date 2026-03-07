# Enrich GraphQL Config Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Store canonical GraphQL request payload + inline SDL in `GraphqlPluginConfig`, validate queries against SDL, and update tests accordingly.

**Architecture:** A dedicated `graphql_payload` module canonicalizes query/document/variables, Base64-encodes SDL, and validates using `graphql_parser`. `GraphqlInteractionBuilder` delegates to the module and embeds the results into `GraphqlPluginConfig`; the server reuses the enriched config and tests assert the new behavior.

**Tech Stack:** Rust (`graphql_parser`, `serde_json`, `base64`), pact_plugin_driver, cargo test harness.

---

### Task 1: Update data structures

**Files:**
- Modify: `pact-graphql-plugin/src/interaction.rs`

**Steps:**
1. Introduce `pact-graphql-plugin/src/graphql_payload.rs` (or module) containing `GraphqlRequestPayload` plus inline SDL metadata structs, and embed the nested payload in `GraphqlPluginConfig` while keeping existing top-level fields for compatibility.
2. Adjust serde derives/implementations to serialize/deserialize both the legacy flat fields and the new nested payload, ensuring the builder/server consumers remain unchanged.

### Task 2: Canonicalization helpers

**Files:**
- Modify: `pact-graphql-plugin/src/interaction.rs`
- Modify: `pact-graphql-plugin/Cargo.toml`

**Steps:**
1. Implement dedent logic (match JS helper), variable canonicalization via `serde_json::Value`, SDL normalization (trim + newline).
2. Add `graphql_parser` dependency (and ensure `base64` already available). Update `Cargo.lock` accordingly.

### Task 3: Validation integration

**Files:**
- Modify: `pact-graphql-plugin/src/interaction.rs`
- Modify: `pact-graphql-plugin/src/server.rs`

**Steps:**
1. Parse SDL + query using `graphql_parser`, build type/field maps, and verify operations/fields exist.
2. Update `GraphqlInteractionBuilder::build` to populate canonical payload + inline schema, returning errors on validation failure.
3. Ensure `config_to_struct` serializes the new fields for pact metadata.

### Task 4: Tests

**Files:**
- Modify: `pact-graphql-plugin/tests/interaction_tests.rs`

**Steps:**
1. Add tests confirming canonical payload + inline schema are present on success.
2. Add test asserting validation failure occurs when query references an unknown field.
3. Run `cargo fmt` and `cargo test --package pact-graphql-plugin tests::interaction_tests`.

### Task 5: Commit

**Files:** n/a

1. Stage affected files (Rust sources, Cargo manifests, tests).
2. Commit with `feat: enrich graphql interaction config`.

---

Plan complete and saved to `docs/plans/2026-03-06-enrich-graphql-config-plan.md`. Which execution mode?
