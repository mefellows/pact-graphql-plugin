# Provider Message Verification Design

## Goal

Make provider message verification exercise real provider code instead of returning a canned JSON payload, while keeping the current vitest-based provider verification flow.

## Context

- `examples/js/product-consumer/verify-provider.test.ts` currently supplies message bodies via a hardcoded object in `messageProviders`.
- The GraphQL provider is implemented in `examples/js/product-consumer/provider-server.ts` using in-memory data and resolvers.
- We want message verification to use the same provider logic that serves live API responses, without introducing a websocket mock yet.

## Decision

Introduce a provider-side helper function for subscription inventory events, use it in both:
- the provider resolvers (so it is exercised by real API queries)
- the message verification provider function (so pact verification uses provider logic)

## Design

### Architecture

- Add a helper (e.g. `buildInventoryChangedEvent`) in `provider-server.ts` that returns the message payload using the same in-memory data as resolvers.
- Wire the helper into `Variant.inventory` (or a subscription-like path) so it is actually used when the API is queried.
- Update `verify-provider.test.ts` `messageProviders` to call the helper instead of returning a literal object.

### Data Flow

- HTTP requests resolve inventory via `Variant.inventory`, which uses the shared helper/data to construct the inventory shape.
- Message verification calls the same helper to produce `data`, `subscription`, and `variables` for the pact message.

### Error Handling

- If a variant ID is not found, the helper should throw (or return null) consistent with current resolver behavior.
- Message verification should surface these errors in the pact verifier output.

## Testing

- Run `npm test` in `examples/js/product-consumer` to regenerate pacts and verify provider.
- Ensure the provider message verification passes with the shared helper path.

## Non-Goals

- No websocket server or subscription transport.
- No changes to the pact message schema or GraphQL schema beyond helper wiring.
