## GraphQL Pact Plugin Design

**Date:** 2026-03-04  
**Author:** OpenCode (with Matt)

### 1. Context & Goals
- Build a first-class Pact plugin so consumers can declare GraphQL interactions without hand-crafting HTTP payloads.
- Focus v1 on solving the request/query pain: normalize JSON vs query-string transports and store canonical GraphQL metadata in Pact files.
- Target JS/TS consumers first, but keep transport-agnostic so other languages can adopt the plugin later.
- Persist optional SDL schemas inside the pact to aid future provider tooling.

### 2. Scope & Non-goals
**In scope**
- Plugin server written in Rust using `pact_plugin_driver` that handles `ConfigureInteraction`, `GenerateContent`, and `VerifyInteraction` calls.
- DSL helper (initially via an npm package) that collects GraphQL fields and invokes the plugin.
- Request encoding for JSON body and query-string transports.
- Storing SDL (when provided) alongside the interaction metadata.

**Out of scope (v1)**
- Schema validation of queries or responses.
- GraphQL fragments/directives support beyond literal documents supplied by the user.
- Provider-side resolver automation or schema fetching via introspection.

### 3. Architecture Overview
- **GraphqlPluginServer** implements Pact plugin traits and delegates to specialized services.
- **GraphqlInteractionBuilder** validates incoming configs, normalizes query documents, and captures metadata (operation name, transport, variables JSON, headers).
- **RequestEncoder** turns the normalized GraphQL payload into either a JSON body (`{"query":...,"variables":...,"operationName":...}`) or URL-encoded query string parameters.
- **SchemaRegistry** stores SDL blobs, computes SHA-256 hashes, and returns lightweight references embedded in pact metadata.
- **GraphqlVerifier** rehydrates stored payloads during verification to ensure replay fidelity (no schema enforcement yet) and exposes schema metadata for future tooling.

All components communicate via in-memory structs; persistent artifacts (schemas) are stored under the plugin's data directory keyed by their hash.

### 3.1 Workspace Bootstrap
- Promote the repository to a Cargo workspace (`resolver = "2"`) with `pact-graphql-plugin` as the initial member so future crates (helpers, examples) slot in cleanly.
- Depend on `pact_plugin_driver = { version = "0.4", package = "pact-plugin-driver" }` so Cargo fetches the hyphenated crate published on crates.io while our code keeps using the snake_case module; the shorthand string form will not resolve.
- Keep the workspace clean with `.gitignore` entries for `target/`, `node_modules/`, `dist/`, `.DS_Store`, and `*.log`.
- `pact-graphql-plugin` is a binary crate whose `src/main.rs` only spins up Tokio and delegates to `server::run()` from `src/lib.rs`, ensuring all plugin logic lives in the library module for reuse by tests.

### 4. Interaction Representation
- Pact interactions gain a `plugin_config.graphql` block with:
  - `query_document`: verbatim GraphQL document exactly as supplied.
  - `operation_name`: optional string.
  - `variables_json`: canonical JSON string of variables.
  - `transport`: `"json_body"` or `"query_string"`.
  - `schema_ref`: `{ "hash": <sha256>, "encoding": "utf-8" }`, plus inline Base64 SDL for portability when provided.
- For JSON transport, plugin serializes standard GraphQL request shape into the HTTP body and ensures `content-type: application/json` unless overridden.
- For query-string transport, plugin URL-encodes `query`, `variables`, and `operationName` params and ensures the mock server path contains the encoded payload.
- Pact core remains GraphQL-agnostic; only the plugin knows how to interpret the embedded metadata.

### 5. Plugin Workflow
1. Consumer DSL invokes plugin with schema (optional), query document, variables JSON/string, operation name, headers, path, and preferred transport.
2. Plugin validates inputs (non-empty query, JSON-parsable variables, allowed transport) and stores metadata.
3. `RequestEncoder` produces the concrete HTTP body or query string and returns it to the Pact builder to finalize the interaction.
4. During verification, the plugin reconstructs the payload exactly as the consumer defined and hands it to the verifier; responses remain standard Pact JSON assertions.

### 6. Schema Handling
- Consumers may pass SDL inline or via helper that reads from disk.
- `SchemaRegistry` canonicalizes whitespace (trim + ensure newline separators) and hashes the result.
- Canonical SDL is persisted in the plugin storage directory as `<hash>.graphql`.
- Pact file embeds both the reference hash and the Base64 SDL; multiple interactions referencing the same schema reuse the hash to avoid duplication.
- Metadata is exposed through `GetMetadata` so provider tooling can download schema artifacts later.

### 7. Consumer Experience (JS/TS v1)
- Helper package: `js/pact-graphql-helper` (published as `@pact-foundation/pact-graphql-helper`) exposes `graphqlInteraction(builder, options)` plus `GraphqlRequestOptions`/`GraphqlTransport` types. It dedents the query, validates/serializes variables, defaults the transport to `json_body`, and forwards the config to `builder.usingPlugin({ pluginName: 'graphql', configuration })`.
- Example usage:
```ts
import { PactV4 } from '@pact-foundation/pact';
import { graphqlInteraction } from '@pact-foundation/pact-graphql-helper';

const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
const interaction = pact.addInteraction('fetch product');
const query = `
  query GetProduct($id: ID!) {
    product(id: $id) {
      id
      name
      type
    }
  }
`;

interaction.given('a product with ID 10 exists');
interaction.uponReceiving('a GraphQL product request');

const pluginInteraction = await graphqlInteraction(interaction, {
  schema: readFileSync('schema.graphql', 'utf8'),
  query,
  variables: { id: '10' },
  operationName: 'GetProduct',
});

pluginInteraction.willRespondWith(200, (builder) => {
  builder.jsonBody({ data: { product: { id: '10' } } });
});
```
- The helper ships with Vitest coverage (JSON body + query-string transport) and is linked into the example consumer via a `file:` dependency until it is published.

### 8. Testing & Examples
- **Rust unit tests**: cover request encoding, variables serialization, schema hashing/embedding, and validation failures (missing query, invalid JSON, unsupported transport).
- **Integration tests (Rust)**: spin up plugin via `pact_plugin_driver` test harness to verify ConfigureInteraction → GenerateContent flow produces deterministic pact files for both transports.
- **Consumer example tests (JS)**: `examples/js/product-consumer/pact.test.ts` uses the helper in a realistic Pact JS test. Run `npm install && npm run test` inside that directory once the GraphQL plugin is installed under `$HOME/.pact/plugins`.
- **Verifier smoke test**: run Pact verifier with the plugin against a mock GraphQL provider to ensure stored payloads replay correctly.

### 9. Risks & Future Work
- No schema validation yet; we plan to reuse stored SDL once operation-level validation is implemented.
- Query documents are stored verbatim; any consumer formatting quirks remain (mitigated by helper normalizing whitespace for deterministic diffs).
- Need to ensure Base64-encoded schemas do not bloat pact files (monitor size, consider compression later).
- Future iterations: schema-aware matchers, fragment support, provider introspection fallback, CLI utilities for schema diffing.
