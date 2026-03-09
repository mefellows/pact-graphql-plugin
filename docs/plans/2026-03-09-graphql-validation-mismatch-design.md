# GraphQL Validation Mismatch Design

## Goal

Ensure schema/query validation failures are surfaced as structured Pact mismatches during `compare_contents` and `verify_interaction`, rather than only at configuration time. This lets Pact JS show actionable errors when running consumer tests or provider verification.

## Scope

### compare_contents

- Run schema + query validation when canonicalizing the expected request.
- On validation failure, return a single `PluginContentMismatch` with:
  - path `/payload/query_document`
  - mismatch `GraphQL query validation failed`
  - diff/details containing the validation error string(s)

### verify_interaction

- Run the same validation for provider-side verification.
- On validation failure, return the same mismatch shape.

### Configuration

- Keep `configure_interaction` validation as-is (so core can improve later), but do not rely on it for user feedback.

## Success Criteria

- Consumer tests fail with a structured mismatch when query/schema validation fails, even if Pact JS does not surface configuration errors.
- Provider verification returns structured mismatches for invalid queries.
