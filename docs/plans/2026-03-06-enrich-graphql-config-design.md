# Enrich GraphQL Config Design

## Context

- Plugin currently stores request fields flatly in `GraphqlPluginConfig` and only writes schema references; we need canonical payloads, inline schema copies, and early query validation to match JS helper expectations.

## Goals

1. Canonicalize and embed the request payload (query, operation name, variables, transport) when configuring interactions.
2. Store a Base64-encoded SDL alongside the registry reference in the pact metadata.
3. Reject mismatched queries by validating operations/fields against the supplied SDL before generating the pact.

## Proposed Changes

### Data Structures
- Introduce `GraphqlRequestPayload { query_document, operation_name, variables_json, transport }`.
- Update `GraphqlPluginConfig` to include `request: GraphqlRequestPayload`, `schema_ref`, and `schema_inline_base64` (Base64 string of canonical SDL).
- Keep `GraphqlPluginRequest` as the wire-facing input struct; `config_to_struct` reuses serde serialization so Pact Core sees the new fields automatically.

### Canonicalization
- **Query:** Split lines, remove leading/trailing blank lines, compute minimum indentation among non-empty lines, and strip that indentation to mimic helper behaviour.
- **Variables:** Parse with `serde_json::Value`, reserialize with `serde_json::to_string` for deterministic ordering.
- **SDL:** `trim()` + append single newline before hashing + Base64 encoding.

### Validation Flow
- Use `graphql_parser` to parse the SDL and query. Build a map of object/interface definitions keyed by name.
- Determine root types (Query/Mutation/Subscription) and recursively traverse selections to ensure each field exists on the parent type.
- On the first unknown type/field, return an `anyhow::Error` describing the missing reference; `configure_interaction` will surface the error to consumers.

### Tests
- Interaction tests assert canonical request payload + inline schema content.
- Add a negative test ensuring unknown field references cause `build` to fail.

### Tooling
- Add `graphql_parser` to `Cargo.toml`.
- Run `cargo fmt` + targeted tests as part of the workflow.
