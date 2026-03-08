# GraphQL Verification Hooks Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable Pact provider verification to reuse canonical GraphQL request matching by wiring `prepare_interaction_for_verification` and `verify_interaction` into the plugin.

**Architecture:** `prepare_interaction_for_verification` returns the stored `GraphqlPluginConfig` so verifiers have the canonical payload. `verify_interaction` deserializes that config, decodes/canonicalizes the provider’s actual HTTP request with the existing helpers, and returns mismatch entries when the canonical payloads differ.

**Tech Stack:** Rust (`graphql_parser`, `serde_json`, Pact plugin proto), existing helper modules, Rust unit tests.

---

### Task 3A: prepare_interaction_for_verification handler

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs`

**Step 1: Deserialize config**
- From `request.get_ref().interaction` pull `interaction_configuration`; return `Status::invalid_argument` if missing. Use `proto_struct_to_json` + `serde_json` to obtain `GraphqlPluginConfig`.

**Step 2: Serialize to response**
- Reuse `config_to_struct(&config)` to create a `prost_types::Struct` and set `VerificationPreparationResponse.plugin_configuration.interaction_configuration` to it.

**Step 3: Tests**
- Add unit test `prepare_interaction_for_verification_returns_config` that builds a fake request with known config and asserts the response reserializes it.

**Step 4: Run fmt/test**
- `cargo fmt`
- `cargo test --package pact_graphql_plugin server::tests::prepare_interaction_for_verification_returns_config -- --exact`

---

### Task 3B: verify_interaction handler

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs`

**Step 1: Deserialize expected config**
- Read `request.get_ref().plugin_configuration.interaction_configuration`; error with `invalid_argument` if missing. Convert into `GraphqlPluginConfig`.

**Step 2: Decode actual HTTP request**
- Extract `request.get_ref().contents` (body + content type); error if missing.
- Call `RequestEncoder::decode(actual_body, content_type, &config.transport)` to obtain `GraphqlRequest`.

**Step 3: Canonicalize / diff**
- Use `CanonicalGraphqlRequest::from_http_request` (with inline SDL or registry lookup) to canonicalize the actual request.
- Build the expected canonical request from `config` (reuse helper) and call `diff`.

**Step 4: Build response**
- Convert mismatches into `proto::verify_interaction_response::InteractionVerificationResult` entries (mirroring compare_contents). Return success when diff empty; mismatches otherwise.

**Step 5: Tests**
- Add tests:
  - `verify_interaction_matches` (no mismatches).
  - `verify_interaction_mismatch` (e.g., query text differs) verifying mismatch path/description.

**Step 6: Run fmt/test**
- `cargo fmt`
- `cargo test --package pact_graphql_plugin server::tests::verify_interaction_mismatch -- --exact`

---

### Task 3C: Wrap-up

**Step 1: Ensure all Task 3 tests pass**
- `cargo test --package pact_graphql_plugin server::tests::prepare_interaction_for_verification_returns_config -- --exact`
- `cargo test --package pact_graphql_plugin server::tests::verify_interaction_matches -- --exact`
- `cargo test --package pact_graphql_plugin server::tests::verify_interaction_mismatch -- --exact`

**Step 2: Document (optional)**
- Note verification support in `docs/plans/2026-03-07-verification-hooks-design.md` if updates needed.
