# JS Invalid Query Tests Design

## Goal

Remove local GraphQL validation from the JS example and rely on plugin mismatch behavior during mock server execution for invalid query cases.

## Context

- `examples/js/product-consumer/pact.test.ts` uses `graphql.validate` locally to fail invalid queries.
- This obscures what the plugin can do and bypasses plugin mismatch behavior.
- The plugin now returns structured mismatches during compare/verify.

## Decision

- Remove `validateQuery` and schema validation helpers from the JS example.
- Keep `graphqlHttpInteraction` for invalid cases, but move failure expectations to the mock server call.

## Design

### Architecture

- Delete imports from `graphql` and remove `validateQuery` logic.
- Update the two invalid schema tests to:
  - configure an interaction via `graphqlHttpInteraction`
  - execute the mock server request with the invalid query
  - `expect(...).rejects.toThrow(...)` with a mismatch message

### Data Flow

- Interaction config still uses plugin contents (schema/query/variables).
- Mock server executes request and yields a mismatch error, which is asserted in the test.

### Testing

- Positive cases unchanged.
- Invalid schema tests assert the error from `executeTest`/mock server call rather than local validation.

## Non-Goals

- No changes to plugin behavior or helper implementation.
- No provider verification changes.
