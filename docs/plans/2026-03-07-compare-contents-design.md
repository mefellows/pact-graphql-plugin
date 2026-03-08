# Compare Contents Runtime Matching Design

## Goal

Implement Task 2 from the runtime matching plan: decode incoming HTTP payloads, canonicalize them alongside stored interaction configs, diff canonical requests, and surface mismatches through Pact's `compare_contents` endpoint.

## Scope

- Extend `RequestEncoder` with a `decode` helper that mirrors existing encoding logic for JSON bodies and query-string transports.
- Implement the full comparison workflow inside `GraphqlPlugin::compare_contents`, reusing `CanonicalGraphqlRequest` utilities and the schema registry held by the plugin builder.
- Add regression tests that simulate matching vs mismatching runtime requests via `compare_contents`.

## Design

### Decode Helper (`encoder.rs`)
- Public API: `fn decode(body: &[u8], content_type: &str, expected_transport: &Transport) -> Result<GraphqlRequest>`.
- Dispatch to `decode_json_body` or `decode_query_string` based on the expected transport (the transport configured when the interaction was created).
- JSON path: deserialize `{ query, operationName, variables }` using Serde, storing `operation_name` as `Option<String>` and `variables_json` as canonicalized JSON via `serde_json::to_string`.
- Query-string path: parse using `form_urlencoded::parse`, capture raw string values, drop empty strings to `None`; leave variable canonicalization to `CanonicalGraphqlRequest::from_http_request`.

### compare_contents (`server.rs`)
1. **Config extraction** – Pull `plugin_configuration.interaction_configuration`, fail with `Status::invalid_argument` if missing, and deserialize to `GraphqlPluginConfig`.
2. **Actual payload** – Require `request.actual` with `content` bytes; propagate `invalid_argument` if absent.
3. **Decode** – Feed the actual body bytes and content type into `RequestEncoder::decode` using the stored `Transport` hint from the config.
4. **Canonicalization** – Build `CanonicalGraphqlRequest` for:
   - Expected side: reuse `config.request` + `config.inline_schema`.
   - Actual side: call `CanonicalGraphqlRequest::from_http_request` with the decoded body bytes, observed content type, stored transport, and inline schema base64 (preferring inline SDL, otherwise fetch via `schema_ref` through the registry embedded in `GraphqlInteractionBuilder`).
5. **Diff + response** – Compare canonical requests via `diff`, convert each `RequestMismatch` into `proto::Mismatch { path, expected, actual, mismatch }`, set `CompareContentsResponse.result` to `Matching` when empty or `Mismatch` when populated.
6. **Errors** – Any decode/canonicalization failure returns `Status::invalid_argument` with the root cause message.

### Testing
- Add unit/integration tests in `server.rs` under `tests` module:
  1. `compare_contents_matches`: build a `GraphqlPluginConfig`, encode an identical HTTP payload, call `compare_contents`, assert `result == Matching` and zero mismatches.
  2. `compare_contents_mismatch`: alter the HTTP request (e.g., different query), assert `result == Mismatch` with the expected `proto::Mismatch` entry.
- Use in-memory schema registry seeded with inline SDL to avoid filesystem I/O during tests.

### Open Questions
- None for Task 2; provider verification flows remain TODO for later tasks.
