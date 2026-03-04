% Request Encoder Design

## Goal

Implement a reusable `RequestEncoder` for the `pact-graphql-plugin` crate that converts structured GraphQL requests into either JSON request bodies or URL-encoded query strings, matching Pact plugin expectations.

## Requirements

- GraphQL request inputs must own their string data to avoid lifetime issues when stored inside plugin metadata.
- Support two transports: JSON body and query-string.
- Optional `operation_name` and `variables_json` should only be emitted when provided.
- JSON serialization must preserve field ordering (`query`, `operationName`, `variables`).
- Query-string output must be URL encoded using `urlencoding::encode`.
- API should expose helper constructors for each transport to keep call sites ergonomic.

## Proposed Structures

- `enum Transport { JsonBody, QueryString }` — public so other modules (future builder/server) can reuse it.
- `struct GraphqlRequest` — owns `String` fields: `query_document`, `operation_name`, `variables_json`, `transport`.
- `impl GraphqlRequest` — helper constructors `json` and `query_string` to build instances with owned data; they normalize optional args to `Option<String>`.
- `struct EncodedRequest` — output payload with `body: Option<String>` and `query_string: Option<String>`; only one is populated per transport for clarity.
- `struct RequestEncoder` — stateless type housing `encode(&GraphqlRequest)` which returns `anyhow::Result<EncodedRequest>`.

## Encoding Logic

### JSON Body

- Use `serde_json::json!` to assemble a map-like structure ensuring key order (`query`, `operationName`, `variables`).
- Skip keys that do not have values.
- Serialize via `serde_json::to_string` and populate `EncodedRequest { body: Some(json_string), query_string: None }`.

### Query String

- Build `Vec<(&str, &str)>` (or owned tuple) with required `query` pair and optional `operationName`/`variables`.
- URL encode each value via `urlencoding::encode`.
- Join with `&` using `format!("{}={}", key, encoded_value)`.
- Return `EncodedRequest { body: None, query_string: Some(result) }`.

## Testing Strategy

- Follow TDD: add failing unit tests under `tests/encoder_tests.rs` covering JSON and query-string transports. Assert exact output strings.
- Ensure optional fields are omitted by expanding test cases later (future work) but initial tests validate base behavior.

## Risks / Mitigations

- **Field ordering**: use explicit `serde_json::json!` field insertion order to guarantee deterministic strings.
- **Encoding correctness**: rely on `urlencoding::encode` rather than manual replacement.
- **Future transports**: keep enum public and `GraphqlRequest` constructors extensible to add headers/body combos later.
