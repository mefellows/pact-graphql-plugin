# GraphQL Runtime Matching Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enforce canonical GraphQL request matching at mock-server and verification time so junk HTTP requests fail Pact executions.

**Architecture:** Reuse the canonicalization/validation helpers to build a `CanonicalGraphqlRequest`, then apply the same diff logic in `compare_contents` and verification endpoints. Both paths deserialize the stored config, canonicalize the actual HTTP request, and surface mismatches with Pact’s standard structures.

**Tech Stack:** Rust (`graphql_parser`, `serde_json`, `tonic`, Pact plugin proto), JS example (Vitest) for end-to-end verification.

---

### Task 1: Extract Canonical Request Helper

**Files:**
- Modify: `pact-graphql-plugin/src/graphql_payload.rs`
- Modify: `pact-graphql-plugin/src/interaction.rs`

**Step 1: Create helper struct**
```rust
pub struct CanonicalGraphqlRequest {
    pub payload: GraphqlRequestPayload,
    pub inline_schema: Option<GraphqlInlineSchema>,
}
```

**Step 2: Move canonicalization functions**
- Relocate `canonicalize_query`, `canonicalize_variables`, `canonicalize_sdl`, and SDL validation helpers into the new module so both configure-time and runtime paths can call them. Ensure existing builder code compiles using the new location.

**Step 3: Add constructors**
- `fn from_interaction_config(req: GraphqlPluginRequest, registry: &SchemaRegistry) -> anyhow::Result<Self>` (wrap existing builder logic).
- `fn from_http_request(body_bytes: &[u8], content_type: &str, expected_transport: Transport, schema_base64: Option<&str>, registry: &SchemaRegistry) -> anyhow::Result<Self>` that decodes JSON/query-string bodies, canonicalizes query/variables, and validates using SDL derived from `schema_base64` or `schema_ref`.

**Step 4: Add diff helper**
```rust
pub struct RequestMismatch { pub path: String, pub expected: String, pub actual: String, pub description: String }
fn diff(&self, other: &CanonicalGraphqlRequest) -> Vec<RequestMismatch>
```

**Step 5: Run `cargo fmt` + `cargo check`**
- Commands:
  - `cargo fmt`
  - `cargo check`

Expected: Compile success.

---

### Task 2: Implement compare_contents

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs`
- Modify: `pact-graphql-plugin/src/encoder.rs` (for decode helper if needed)

**Step 1: Add HTTP decode helper**
- In `encoder.rs`, add `fn decode(body: &[u8], transport: &Transport) -> anyhow::Result<GraphqlRequest>` to invert the existing encoder (parse JSON or query string into query/operation/variables).

**Step 2: Update compare_contents**
- In `GraphqlPlugin::compare_contents`, perform:
  1. Deserialize `GraphqlPluginConfig` from `request.plugin_configuration`.
  2. Decode actual HTTP body via `RequestEncoder::decode`.
  3. Build canonical actual request using `CanonicalGraphqlRequest::from_http_request` with inline SDL or schema ref (fetch from registry if needed).
  4. Diff expected vs actual canonical requests.
  5. Populate `CompareContentsResponse` mismatches (convert `RequestMismatch` into `proto::Mismatch` entries). Return success when diff empty.

**Step 3: Add unit/integration tests**
- In `pact-graphql-plugin/src/server.rs` (or a new test module), simulate matching and mismatching requests by constructing configs + actual payloads, invoking `compare_contents`, and asserting mismatch output.

**Step 4: Run tests**
- Commands:
  - `cargo fmt`
  - `cargo test --package pact_graphql_plugin server::tests::compare_contents_matches -- --exact`
  - `cargo test --package pact_graphql_plugin server::tests::compare_contents_mismatch -- --exact`

Expected: Passing.

---

### Task 3: Implement verification hooks

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs`

**Step 1: prepare_interaction_for_verification**
- Deserialize config, serialize canonical payload + inline schema into `VerificationPreparationResponse::plugin_configuration` so verifiers can reuse it.

**Step 2: verify_interaction**
- Accept provider-supplied request/response data (per proto), canonicalize actual request with the helper, diff vs expected, and return mismatches in `VerifyInteractionResponse`. Ensure success path includes any needed metadata for Pact Core.

**Step 3: Add tests**
- Unit tests covering verification success/failure cases by stubbing proto requests.

**Step 4: Run targeted tests**
- Commands:
  - `cargo fmt`
  - `cargo test --package pact_graphql_plugin server::tests::verify_interaction_mismatch -- --exact`

---

### Task 4: JS example + end-to-end validation

**Files:**
- Modify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Update test expectations**
- Wrap the `executeTest` call in `await expect(...)` and assert it rejects with a mismatch containing e.g. `field  is not defined` when sending junk query.

**Step 2: Run example tests**
- Commands:
  - `cd examples/js/product-consumer`
  - `npm install` (if needed)
  - `npm test` (or `npx vitest run pact.test.ts`)

Expected: Tests now fail with mismatches when HTTP request is invalid (assert using `rejects.toThrow`).

**Step 3: Document behavior (optional)**
- Update `README.md` or design docs summarizing runtime matching change.

---

### Task 5: Final verification & commit

**Step 1: Run full crate tests**
- `cargo fmt`
- `cargo test --package pact_graphql_plugin`

**Step 2: git status / diff**
- Ensure only intended files changed.

**Step 3: Commit**
- `git add <all relevant files>`
- `git commit -m "feat: enforce graphql runtime matching"`

**Step 4: Instructions for PR**
- Mention new runtime enforcement + verification hooks, updated JS example.
