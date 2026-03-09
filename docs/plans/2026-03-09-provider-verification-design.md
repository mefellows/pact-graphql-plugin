# Provider Verification as Vitest

## Goal

Refactor the provider verification script into a vitest test that verifies both HTTP and message interactions in one run, while keeping negative scenarios isolated in a separate PactV4 provider name so they do not affect verification.

## Context

- The current `verify-provider.ts` uses `filterPactInteractions` to remove a deliberately failing interaction ("extra response fields").
- This exists because the verifier must only see interactions expected to pass. Negative scenarios are instructional and should not be verified against the real provider.
- We now also produce message pacts, and we want the provider verification test to be a pedagogical example that verifies both HTTP and messages together.

## Decision

- Convert `verify-provider.ts` into a vitest test file (`verify-provider.test.ts`).
- Use a single `it(...)` that runs the verifier once for both HTTP and message pacts.
- Move negative scenarios to a separate PactV4 setup with a different provider name (e.g. `product-provider-negative`).
- Remove `filterPactInteractions` entirely.

## Design

### Architecture

- `beforeAll`: start the provider server and capture `providerBaseUrl`.
- `afterAll`: stop the server.
- Single `it(...)`:
  - Configure `Verifier` with `pactUrls` pointing only to the valid pact files (HTTP + message).
  - Provide `messageProviders` so pact-js can verify message interactions alongside HTTP.

### Data Flow

- HTTP interactions use the GraphQL provider server via `providerBaseUrl`.
- Message interactions use a message provider function that returns the subscription payload used in the pact.
- Pact files are read directly from the `examples/js/product-consumer/pacts` directory (or specific message pact location).

### Error Handling

- Let verifier errors surface directly in vitest output.
- Avoid bespoke filtering to keep the example clear and minimal.

## Testing

- Update `examples/js/product-consumer/package.json` `verify` script to run `vitest run verify-provider.test.ts`.
- Running `npm run verify` should now verify both HTTP and message interactions in one test.

## Non-Goals

- No changes to provider implementation or schema.
- No attempt to verify negative interactions; they remain educational only.
