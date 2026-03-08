# Compare Contents Runtime Matching Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable Pact mock-server matching for GraphQL interactions by decoding HTTP payloads, canonicalizing actual requests, and diffing them against stored interaction configs.

**Architecture:** Extend the encoder with a decode helper, wire `GraphqlPlugin::compare_contents` to canonicalize + diff requests via `CanonicalGraphqlRequest`, and add high-level tests that hit the compare API with matching/mismatching payloads.

**Tech Stack:** Rust, Tokio, tonic, serde_json, pact-plugin-driver, cargo test/fmt.

---

### Task 1: Implement RequestEncoder::decode

**Files:**
- Modify: `pact-graphql-plugin/src/encoder.rs`
- Test: `cargo test --package pact_graphql_plugin encoder::tests::decode_*` (add new tests if needed)

**Step 1: Write failing unit tests**

```rust
#[test]
fn decode_json_body_roundtrip() { /* ensure JSON decode matches encode */ }

#[test]
fn decode_query_string_roundtrip() { /* ensure query-string decode matches encode */ }
```

**Step 2: Run focused tests to see failures**

Run: `cargo test --package pact_graphql_plugin encoder::tests::decode_json_body_roundtrip -- --exact`
Expected: Fails because decode helper not implemented.

**Step 3: Implement decode helper**

- Add `RequestEncoder::decode` + `decode_json_body` + `decode_query_string` as per design doc.
- Reuse `serde_json`, `form_urlencoded`, and `percent_decode` helpers; ensure empty strings become `None`.

**Step 4: Re-run tests**

`cargo test --package pact_graphql_plugin encoder::tests::decode_json_body_roundtrip encoder::tests::decode_query_string_roundtrip -- --exact`
Expected: PASS.

**Step 5: cargo fmt**

`cargo fmt`

### Task 2: Implement GraphqlPlugin::compare_contents

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs`
- Reference: `pact-graphql-plugin/src/graphql_payload.rs`, `pact-graphql-plugin/src/interaction.rs`

**Step 1: Write integration test scaffolding**

Add tests under `server.rs::tests` for `compare_contents_matches` and `compare_contents_mismatch` that construct configs + payloads.

**Step 2: Run targeted tests to confirm they fail**

`cargo test --package pact_graphql_plugin server::tests::compare_contents_matches server::tests::compare_contents_mismatch -- --exact`
Expected: FAIL because compare_contents returns default response.

**Step 3: Implement compare_contents workflow**

- Deserialize `GraphqlPluginConfig`.
- Retrieve actual body/content-type, call `RequestEncoder::decode`.
- Build canonical expected/actual requests via `CanonicalGraphqlRequest` utilities and registry from builder.
- Diff mismatches, map into `proto::Mismatch`, set response result accordingly, propagate errors via `Status::invalid_argument`.

**Step 4: Re-run targeted tests**

`cargo test --package pact_graphql_plugin server::tests::compare_contents_matches server::tests::compare_contents_mismatch -- --exact`
Expected: PASS.

### Task 3: Full formatting + regression suite

**Files:**
- Entire workspace (fmt + targeted tests)

**Step 1: cargo fmt**

`cargo fmt`

**Step 2: Run full specified tests**

`cargo test --package pact_graphql_plugin server::tests::compare_contents_matches -- --exact`

`cargo test --package pact_graphql_plugin server::tests::compare_contents_mismatch -- --exact`

**Step 3: Review `git status`**

`git status -sb`

**Step 4: (Deferred) Commit once user requests**

`git commit -am "feat: implement GraphQL compare_contents"`

---

Plan complete and saved to `docs/plans/2026-03-07-compare-contents-plan.md`. Two execution options:

1. **Subagent-Driven (this session)** – I spin up a fresh subagent per task with reviews between steps for rapid iteration.
2. **Parallel Session (separate)** – Open a new session using superpowers:executing-plans to implement tasks in sequence.

Which approach?
