## Overview

Move the request canonicalization helpers out of `interaction.rs` and into
`graphql_payload.rs` so that configure-time builders and future runtime
comparison/verification paths share the same logic. Introduce a canonical request
struct that captures the normalized payload alongside any inline schema material
needed for validation.

## Goals

1. Define a reusable `CanonicalGraphqlRequest` abstraction with validation-ready
   data.
2. Provide constructors from both plugin interaction configs and incoming HTTP
   requests, delegating to shared canonicalization helpers.
3. Supply a lightweight diff helper that reports mismatches using JSON pointer
   paths for future verification features.

## Non-Goals

* Implement runtime comparison logic (will use the new helpers later).
* Change how schemas are stored or retrieved in the registry.

## Proposed Changes

### New Types

* `CanonicalGraphqlRequest` stores the canonical `GraphqlRequestPayload` and the
  resolved inline schema (if any). Lives in `graphql_payload.rs` next to the
  existing payload structs.
* `RequestMismatch` captures a JSON pointer `path`, stringified `expected` and
  `actual` values, plus a descriptive message. Returned by diff operations.

### Helper Functions

Relocate the following helpers from `interaction.rs` into
`graphql_payload.rs`, keeping their behavior unchanged so existing callers keep
working:

* `canonicalize_query`
* `canonicalize_variables`
* `canonicalize_sdl`
* newline dedent utilities (`dedent_and_trim`, `strip_indent`,
  `normalize_newlines`)
* GraphQL validation helpers (`validate_query_document` and friends), along with
  `SchemaIndex` et al., so constructors can validate requests after loading
  schemas from the registry.

`interaction.rs` will import the canonical helpers via the new struct rather
than keeping local copies.

### Constructors

* `CanonicalGraphqlRequest::from_interaction_config(req, registry)` wraps the
  existing `GraphqlInteractionBuilder` logic: canonicalize query/variables, load
  schema from `req.schema_sdl`/`inline_schema`/`schema_ref`, validate, store in
  registry as needed, and populate `inline_schema` metadata.
* `CanonicalGraphqlRequest::from_http_request(body_bytes, content_type,
  expected_transport, schema_base64, registry)` handles incoming HTTP pact
  verifications by decoding the request body according to transport
  (JSON/GraphQL form), canonicalizing fields, retrieving schemas via
  `schema_base64` or registry references, and performing validation.

### Diff Helper

Implement `CanonicalGraphqlRequest::diff(&self, other)` that compares query
document, operation name, variables JSON, transport, and inline schema contents.
Each mismatch yields a `RequestMismatch` with JSON pointer paths like
`/payload/query_document` or `/inline_schema/base64_sdl`.

## Testing & Validation

* `cargo fmt` after refactor.
* `cargo check` to ensure builders, constructors, and callers still compile.
* Existing unit/integration coverage (if any) continues to exercise serialization
  and builder behavior; further runtime compare tests will arrive with future
  tasks.
