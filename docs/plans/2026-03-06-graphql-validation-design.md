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

#### 1.2 Schema embedding + validation
- When `schema_sdl` is provided, canonicalize and Base64-encode the text, storing it inline alongside the hash reference.
- Parse the SDL + query using `graphql_parser`, ensure operations/fields exist, and error out on invalid documents during `configure_interaction`.

### 2. Helper redesign
- Provide a high-level API `graphqlInteraction({ pact, description, given, schema, query, variables, operationName, expected })` that creates the interaction, sets headers + plugin metadata, wires the response, and runs `executeTest`. Consumers only supply test data; helper handles Pact boilerplate.

### 3. Pact schema metadata
- Serialize inline SDL into pact metadata so provider tooling can diff schemas straight from the pact file.

## Alternatives Considered
- Partial helper abstraction (request only) – rejected; still too much duplication.
- Client-side validation – insufficient for other languages; plugin validation keeps semantics central.

## Risks & Mitigations
- GraphQL parsing complexity – mitigate with well-tested crates + targeted unit tests.
- Backward compatibility – version helper as 0.2.0 and document migration; plugin remains backward compatible at the protocol level.

## Next Steps
1. Implement plugin changes (canonical payload + validation).
2. Add request comparison and mismatches.
3. Redesign helper API, update tests/examples.
4. Document workflow and run smoke tests.
