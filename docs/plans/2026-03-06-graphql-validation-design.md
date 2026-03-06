# GraphQL Plugin Validation & Helper Redesign

**Date:** 2026-03-06  
**Author:** OpenCode (with Matt)

## Goals
- Ensure Pact mock server verifies that the consumer sends the exact GraphQL operation + variables declared during interaction configuration.
- Embed canonical SDL inside the pact so provider tooling can compare schemas without fetching external artifacts.
- Expand `@pact-foundation/pact-graphql-helper` so consumers no longer have to wire `withRequest`/`willRespondWith` manually; a single helper call should configure the plugin request, response, and execution harness.

## Requirements Recap
1. **Request validation** – plugin should compare incoming HTTP body against the stored interaction (query document, operation name, variables, transport). Tests should fail if the consumer sends anything different.
2. **Helper ergonomics** – API should feel like `graphqlInteraction(testConfig)` returning an object that handles request/response wiring and executes the Pact test. Consumers supply schema, query, variables, operation name, and expected response body.
3. **Schema embedding** – pact metadata should include canonicalized SDL (Base64) alongside existing `schema_ref` so provider-side tests can detect schema drift later.
4. **Schema validation** – plugin validates the provided GraphQL document against the SDL during `configure_interaction` to catch mismatched fields early.

## Approach

### 1. Plugin-side changes
#### 1.1 Request capture & validation
- Extend `configure_interaction` to persist the canonical GraphQL request payload (dedented document, sorted variables JSON, transport info) inside `GraphqlPluginConfig`.
- Update `compare_contents` to parse both expected config and actual request body (JSON or query-string). If mismatched, return a structured mismatch (expected vs actual) so Pact surfaces it without crashing.
- During `generate_content`, re-use the stored request payload to populate the mock response body; no changes needed.

#### 1.2 Schema embedding + validation
- When `schema_sdl` is provided, canonicalize (trim, ensure trailing newline) and compute hash as before, but also Base64-encode the text and store it directly under a new field (e.g., `schema_inline_base64`) inside `GraphqlPluginConfig`.
- During `configure_interaction`, parse the SDL and validate the provided GraphQL document (ensure operation exists, fields are valid). Use a lightweight Rust GraphQL parser to build the AST and walk it against the SDL.
- If parsing or validation fails, return an error so the consumer test fails immediately.

### 2. Helper redesign
#### 2.1 API shape
- Export a single `graphqlInteraction` function that accepts options:
  ```ts
  graphqlInteraction({
    pact,
    description,
    schema,
    query,
    variables,
    operationName,
    given,
    expected,
  })
  ```
  where `expected` includes response status/headers/body. The helper internally calls `pact.addInteraction(description)` and wires `withRequest`/`willRespondWith` + `executeTest`.
- Return the promise from `executeTest` so consumers can `await graphqlInteraction(...)` in their tests without touching lower-level pact APIs.

#### 2.2 Response body handling
- Accept `expected.responseBody` as JSON (or builder callbacks for advanced matchers). For MVP, support JSON object + optional headers/status; helper translates to `builder.jsonBody` and `builder.headers`.
- Optionally support Pact matchers (type, regex) in the JSON through `@pact-foundation/pact` matchers; helper should detect matcher types and pass them through untouched.

### 3. Pact schema metadata
- Extend `config_to_struct` to include `schema_inline_base64` so interactions carry SDL inline.
- Update README + example test to mention the pact now contains schema data for provider drift detection.

## Alternatives Considered
- **Helper partial abstraction** – only wrap the request, leaving response wiring to the user. Rejected: duplicates boilerplate, doesn’t meet the “full abstraction” request.
- **Schema validation in JS helper** – early feedback but doesn’t cover other language clients; plugin-level validation ensures consistency across ecosystems.
- **Client-side request assertions** – could intercept `fetch` and compare payloads before hitting pact, but plugin-level validation reduces duplication and keeps the mock server authoritative.

## Risks & Mitigations
- **Parsing SDL + queries in Rust** – need a reliable parser (e.g., `graphql-parser` crate). Mitigation: choose a battle-tested crate, add unit tests.
- **Backward compatibility** – existing helper usage will break once API changes. Mitigation: version helper as `0.2.0` with release notes, keep new API clearly documented.
- **Checksum/pact size** – embedding Base64 SDL increases pact file size. Mitigation: store canonical text (trimmed, deduplicated via `schema_ref`) to minimize duplication.

## Next Steps
1. Implement plugin validation + schema embedding.
2. Redesign helper API + update example test.
3. Extend README/docs to cover new workflow.
4. Update tests (Rust + JS) to cover validation and helper behavior.
