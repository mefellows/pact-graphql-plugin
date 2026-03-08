# GraphQL Runtime Matching Design

## Context

- Tasks 1–4 enriched configure-time behavior (canonical payload + SDL validation) but runtime hooks remain no-ops: `compare_contents` always succeeds and verification endpoints are unimplemented, so Pact tests pass even when HTTP requests are junk.
- JS helper still duplicates query/schema data and relies on Pact core to enforce matching, which it can’t do without plugin compare/verify logic.

## Goals

1. Enforce canonical request matching during mock-server execution via `compare_contents`.
2. Reuse the same canonical comparison during provider verification (`prepare_interaction_for_verification` / `verify_interaction`).
3. Keep canonicalization/validation centralized so configure, mock, and verification flows stay aligned.

## Proposed Changes

### Canonical Request Module
- Extract existing helpers into a reusable API (e.g., `graphql_payload::canonical`):
  - `CanonicalGraphqlRequest { payload: GraphqlRequestPayload, inline_schema: Option<GraphqlInlineSchema> }`.
  - `from_interaction_config(GraphqlPluginRequest)` for configure-time use.
  - `from_http_request(body_bytes, content_type, schema_base64)` to decode JSON/query-string bodies, canonicalize query/variables, validate against SDL, and build the canonical payload.
- Provide `diff(&self, other) -> Vec<Mismatch>` returning structured differences (field path, expected, actual, message).

### Mock-Server Comparison (`compare_contents`)
- Steps:
  1. Deserialize `GraphqlPluginConfig` from `plugin_configuration`.
  2. Decode actual HTTP request via inverse `RequestEncoder` based on stored transport.
  3. Build canonical actual request with `from_http_request`, using inline SDL from config (or fetching from registry via `schema_ref`).
  4. Diff expected vs actual canonical payload + inline SDL; produce Pact `mismatches` entries when fields differ (query, operation name, variables JSON, inline schema, transport).
  5. Return success only when diff is empty; otherwise include mismatch descriptions.

### Verification Flow
- `prepare_interaction_for_verification`: attach stored canonical payload / inline schema to the response (serialized JSON) so verifiers have context.
- `verify_interaction`: accept provider-supplied request data, canonicalize via `from_http_request`, reuse the diff helper, and populate `VerifyInteractionResponse` with mismatch info mirroring `compare_contents` so provider verification fails on divergence.

### Testing & Tooling
- Rust tests:
  - Unit tests for canonical diffing (match/mismatch scenarios).
  - Integration-style tests invoking `compare_contents` with matching/mismatching payloads.
  - Tests for `verify_interaction` handling mismatches and successes.
- JS example update (`examples/js/product-consumer/pact.test.ts`) to assert junk HTTP requests now cause Pact to fail with a mismatch.
