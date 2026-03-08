# GraphQL Verification Hooks Design

## Context

- Runtime matching (`compare_contents`) now canonicalizes HTTP requests and reports Pact mismatches, but provider verification still relies on stubbed `prepare_interaction_for_verification` / `verify_interaction` handlers.
- Without verification hooks, Pact provider tests can’t leverage the canonical payloads to reject junk provider requests/responses.

## Goals

1. `prepare_interaction_for_verification` must return the stored canonical config so verifiers have full context without re-running configuration.
2. `verify_interaction` must reuse the canonical diff logic from `compare_contents`, producing structured mismatches when provider traffic doesn’t match expectations.
3. Maintain consistent error handling (missing data → `invalid_argument`) and mismatch formatting across mock and verification phases.

## Proposed Changes

### Preparation Endpoint
- Deserialize `GraphqlPluginConfig` from `request.interaction.interaction_configuration` (error if absent).
- Reuse `config_to_struct` to serialize the config into a `prost_types::Struct`.
- Return the struct within `VerificationPreparationResponse.plugin_configuration`. No additional metadata required yet.

### Verification Endpoint
- Deserialize `GraphqlPluginConfig` from `request.plugin_configuration.interaction_configuration` (error if absent).
- Extract the provider’s actual HTTP request body/content type from `request.contents` (error if missing).
- Decode and canonicalize the actual request using `RequestEncoder::decode` + `CanonicalGraphqlRequest::from_http_request`, leveraging inline SDL or registry lookup to validate against the schema.
- Diff canonical actual vs expected payloads via `CanonicalGraphqlRequest::diff` and convert mismatches into `proto::Mismatch` entries; respond with success when diff empty, mismatches otherwise.

### Testing
- Add unit tests covering:
  - `prepare_interaction_for_verification` returns the serialized config.
  - `verify_interaction` success (matching request) and failure (mismatched query, missing variables, etc.).
- Reuse existing helper builders for configs and CanonicalGraphqlRequest to keep tests concise.
