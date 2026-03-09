# Go GraphQL Example + Wrapper Design

## Goal

Create a Go example that mirrors the JS product-consumer project (HTTP pact, message pact, provider server, provider verification) and a reusable Go wrapper package for GraphQL pact interactions.

## Context

- The JS example in `examples/js/product-consumer` demonstrates HTTP + message pacts and provider verification.
- We want a Go example to validate that the plugin flow is not JS-specific.
- A small Go wrapper package should simplify usage and align with the JS helper API.

## Decision

- Add a reusable Go package at `go/pactgraphql`.
- Add a single-module Go example at `examples/go/product-consumer` that imports `go/pactgraphql` via `replace`.
- Mirror the JS example structure and schema/data behavior.

## Design

### Architecture

- `go/pactgraphql` (wrapper package)
  - `HTTPInteraction(...)` helper to configure Pact Go V4 interaction with GraphQL plugin contents.
  - `MessageInteraction(...)` helper to configure Pact Go V4 async message interaction.
  - Utilities for GraphQL request body building and schema loading.

- `examples/go/product-consumer` (single Go module)
  - Consumer tests for HTTP and message pacts.
  - Provider server using a Go GraphQL library with the same schema/data as JS.
  - Provider verification test that verifies both HTTP and message pacts.
  - `go.mod` uses `replace` to point at local `go/pactgraphql`.

### Data Flow

- Consumer tests generate pacts under `examples/go/product-consumer/pacts`.
- Provider verification reads those pacts and uses provider logic to create message payloads.
- Provider server uses in-memory data mirroring the JS example.

### Error Handling

- Wrapper returns errors when schema or query input is invalid.
- Provider message builder uses existing provider data and fails fast for unknown IDs.

### Testing

- `go test ./...` in `examples/go/product-consumer` should run:
  - HTTP pact test
  - Message pact test
  - Provider verification test
- Optional unit tests for `go/pactgraphql` helpers.

## Non-Goals

- No websocket subscription server yet.
- No Go workspace (`go.work`) required.
- No changes to core plugin logic.
