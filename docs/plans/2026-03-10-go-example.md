# Go GraphQL Example Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Go example that mirrors the JS GraphQL pact flow (HTTP + message pacts + provider verification) and add a reusable Go wrapper package for GraphQL pact interactions.

**Architecture:** Add `go/pactgraphql` as a reusable helper package and `examples/go/product-consumer` as a single Go module that imports it via `replace`. Implement consumer tests, provider server, and provider verification to match the JS example behavior.

**Tech Stack:** Go, Pact Go V4, Go GraphQL library (gqlgen or graphql-go), standard net/http testing.

---

### Task 1: Create Go wrapper package scaffold

**Files:**
- Create: `go/pactgraphql/go.mod`
- Create: `go/pactgraphql/graphql.go`
- Create: `go/pactgraphql/message.go`

**Step 1: Write the failing test**

Add a minimal Go test in `go/pactgraphql` asserting the helper compiles and builds a request payload:

```go
func TestGraphQLRequestBody(t *testing.T) {
  body, err := GraphQLRequestBody("query { __typename }")
  if err != nil { t.Fatal(err) }
  if !strings.Contains(string(body), "__typename") { t.Fatalf("missing query") }
}
```

**Step 2: Run test to verify it fails**

Run: `go test ./...`
Expected: FAIL (helper not implemented).

**Step 3: Write minimal implementation**

Implement in `graphql.go`:

```go
package pactgraphql

func GraphQLRequestBody(query string, variables map[string]any, operationName string) ([]byte, error) {
  // JSON encode {query, variables, operationName}
}
```

Add helper for plugin contents struct used by Pact Go V4.

**Step 4: Run test to verify it passes**

Run: `go test ./...`
Expected: PASS.

**Step 5: Commit**

```bash
```

### Task 2: Implement HTTP interaction helper

**Files:**
- Modify: `go/pactgraphql/graphql.go`
- Create: `go/pactgraphql/http_interaction.go`

**Step 1: Write the failing test**

Add a test that uses Pact Go V4 to build an interaction with plugin contents from the helper (without executing a real test yet).

**Step 2: Run test to verify it fails**

Run: `go test ./...`
Expected: FAIL until helper implemented.

**Step 3: Write minimal implementation**

Implement a function like:

```go
func HTTPInteraction(p *pact.V4, name string, cfg GraphQLInteractionConfig) (*pact.Interaction, error)
```

Populate plugin contents (schema, query, variables, transport) to mirror JS helper behavior.

**Step 4: Run test to verify it passes**

Run: `go test ./...`
Expected: PASS.

**Step 5: Commit**

```bash
```

### Task 3: Implement message interaction helper

**Files:**
- Modify: `go/pactgraphql/message.go`

**Step 1: Write the failing test**

Add a test that builds a Pact Go V4 async message interaction with plugin contents.

**Step 2: Run test to verify it fails**

Run: `go test ./...`
Expected: FAIL.

**Step 3: Write minimal implementation**

Implement:

```go
func MessageInteraction(p *pact.V4, name string, cfg GraphQLMessageConfig) (*pact.Interaction, error)
```

**Step 4: Run test to verify it passes**

Run: `go test ./...`
Expected: PASS.

**Step 5: Commit**

```bash
```

### Task 4: Scaffold Go example module

**Files:**
- Create: `examples/go/product-consumer/go.mod`
- Create: `examples/go/product-consumer/README.md`
- Create: `examples/go/product-consumer/schema.graphql`

**Step 1: Write the failing test**

Add a placeholder Go test that imports the local helper and fails until module wiring works.

**Step 2: Run test to verify it fails**

Run: `go test ./...` in `examples/go/product-consumer`
Expected: FAIL (module not wired).

**Step 3: Write minimal implementation**

Add `go.mod` with `replace` to `../../go/pactgraphql` and copy the JS schema into `schema.graphql`.

**Step 4: Run test to verify it passes**

Run: `go test ./...`
Expected: PASS.

**Step 5: Commit**

```bash
```

### Task 5: Implement consumer HTTP pact test

**Files:**
- Create: `examples/go/product-consumer/pact_http_test.go`

**Step 1: Write the failing test**

Add a pact test using the Go helper with a simple query (matching JS example) and assert pact file output.

**Step 2: Run test to verify it fails**

Run: `go test ./...`
Expected: FAIL until helper usage is correct.

**Step 3: Write minimal implementation**

Implement the pact test to generate `pacts/product-consumer-product-provider.json`.

**Step 4: Run test to verify it passes**

Run: `go test ./...`
Expected: PASS.

**Step 5: Commit**

```bash
```

### Task 6: Implement message pact test

**Files:**
- Create: `examples/go/product-consumer/pact_message_test.go`

**Step 1: Write the failing test**

Add a message pact test using the Go helper.

**Step 2: Run test to verify it fails**

Run: `go test ./...`
Expected: FAIL.

**Step 3: Write minimal implementation**

Implement the message pact test to generate `pacts/messages/product-consumer-product-provider.json`.

**Step 4: Run test to verify it passes**

Run: `go test ./...`
Expected: PASS.

**Step 5: Commit**

```bash
```

### Task 7: Implement provider server + verification

**Files:**
- Create: `examples/go/product-consumer/provider_server.go`
- Create: `examples/go/product-consumer/verify_provider_test.go`

**Step 1: Write the failing test**

Create a verification test that loads the HTTP + message pact files and fails until provider server exists.

**Step 2: Run test to verify it fails**

Run: `go test ./...`
Expected: FAIL.

**Step 3: Write minimal implementation**

Implement provider server with the same schema and in-memory data as the JS example. Provide a helper to build the message payload and use it in verification.

**Step 4: Run test to verify it passes**

Run: `go test ./...`
Expected: PASS.

**Step 5: Commit**

```bash
  examples/go/product-consumer/verify_provider_test.go
```

### Task 8: End-to-end sanity

**Step 1:** Run `go test ./...` in `examples/go/product-consumer`.

Expected: PASS.

**Step 2:** Commit if any fixes were required.
