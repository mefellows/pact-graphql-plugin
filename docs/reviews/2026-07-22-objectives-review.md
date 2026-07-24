# GraphQL Plugin — Objectives Review

Date: 2026-07-22
Scope: `pact-graphql-plugin/` (Rust core), `js/pact-graphql-helper/`, `go/pactgraphql/`, `examples/{js,go}/product-consumer/`

## Scorecard

| # | Objective | Verdict |
|---|-----------|---------|
| 1 | DX for GraphQL clients is beautiful and easy to use | **Not met** |
| 2 | Core plugin works across languages | **Partially met** — right idea, wrong seam |
| 3 | TS/Go interface does no heavy lifting, just improves DSL | **Not met** |
| 4 | Adds value beyond the raw Pact DSL | **Marginal today** |

The core is architecturally sound and the schema/validation machinery in Rust is real work
that's worth keeping. The problems are all at the boundary: what the plugin *owns* vs what
the bindings own, and the fact that the plugin currently ignores the half of the interaction
where GraphQL awareness matters most (the response).

---

## 1. DX — Not met

### 1.1 The user writes the query, variables and operation name twice

`examples/js/product-consumer/pact.test.ts:50-78`:

```ts
const responseInteraction = await graphqlHttpInteraction(interaction, {
  schema, query, variables: { id: '10' }, operationName: 'GetProduct',
});
// ...
.executeTest(async (mockServer) => {
  await postGraphqlRequest(mockServer, graphqlRequestBody({
    query, variables: { id: '10' }, operationName: 'GetProduct',   // ← same trio again
  }));
});
```

Every example repeats this. The two copies can silently drift (and the "runtime mismatch"
test at `pact.test.ts:377` deliberately exploits that drift — which is fine as a negative
test but shows the API's default shape invites the bug). Same pattern in Go at
`examples/go/product-consumer/pact_http_test.go:53-77`.

### 1.2 The user must know plugin-internal magic

- `content-type: application/graphql` must be set by hand on the runtime request, with a
  comment in the example explaining why (`pact.test.ts:37-38`). The helper sets it on the
  expectation side but the user sets it on the sending side.
- The path `/graphql` is hardcoded in `graphqlHttpInteraction` (`index.ts:130`) and *also*
  hardcoded by the user in `fetch` — and there is no way to override it for services that
  serve GraphQL at `/api/graphql` or `/v1/query`.
- `process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0'` appears at the top of every example
  file. Plugin version resolution via environment variable is not something a consumer
  author should ever see.

### 1.3 The two headline features do not actually fail a test

This is the most serious DX finding. From the example's own comments:

```ts
// pact-js doesn't surface plugin validation errors, so validate queries locally.
const validateQuery = (source: string) => validate(graphqlSchema, parse(source));
```
(`pact.test.ts:23-24`)

```ts
// Pact JS doesn't surface response schema violations; validate locally.
const unknownFields = getUnknownFields('Product', responseBody.data.product);
```
(`pact.test.ts:475-476`)

So the "schema invalid" tests (`pact.test.ts:336-375`) never touch the plugin — they call
`graphql`'s own `validate()` directly. And the "response not in schema" test
(`pact.test.ts:449`) asserts the violation locally and then **writes a pact containing an
invalid response anyway**. A user reading these examples reasonably concludes the plugin
doesn't do the thing it exists to do.

Root causes are two different bugs:
- Consumer-side validation errors do fire in Rust (`graphql_payload.rs:98`) but are returned
  as a gRPC `Status` from `configure_interaction` (`server.rs:219`), which the Pact JS/Go
  drivers appear to swallow. Needs a fix in the drivers, plus a fail-fast fallback in the
  helpers so the error is *always* good.
- Response validation doesn't exist at all — see §4.1.

**Update (2026-07-24, `docs/plans/2026-07-24-response-part-wiring.md`).** The second bullet is
now out of date: response validation exists and runs. `graphqlHttpInteraction` accepts a
`response` option that makes a second, response-side `configure_interaction` call to the
plugin. That call validates the response body against the schema and the query's selection
set and, on success, derives Pact matching rules from the schema (scalar/enum types) and
attaches them to the response part, which Pact core's own JSON matcher then enforces. This is
verified end-to-end: the examples in `pact.test.ts` now build their happy-path responses via
`response` rather than raw `jsonBody`, the generated pact file carries the derived
`matchingRules` on the response body, and provider verification against the example's real
Express server still passes.

The first bullet's underlying problem, however, is confirmed and still open, and it now also
covers the response side. A spike (`## Task 1 findings` in the plan above) proved that when
the response-side `configure_interaction` call itself returns an `Err` (e.g. the *expected*
response in the test contains a field absent from the schema), the plugin's rejection reaches
`pact_ffi` correctly, but `@pact-foundation/pact-core`'s
`withPluginResponseInteractionContents` hardcodes `return true` and discards the FFI error
code, so the JS caller never sees it — the interaction is silently recorded with a bodyless
response part instead of failing the test. So: **response matching now genuinely works**
(a response that violates the schema-derived rules fails the interaction at comparison time);
**response rejection at author time** (the author declaring an expected response that itself
violates the schema) **still does not surface**, for the same class of upstream `pact-core`
bug already identified for query validation. The "response not in schema" example
(`pact.test.ts`) therefore still validates locally with a `TODO` marker rather than asserting
on the plugin, and should not be read as evidence the gap is closed.

### 1.4 Subscriptions / messages are half-built

- The envelope shape `{ subscription, variables, data }` is invented in the JS helper
  (`index.ts:152-159`), not by the plugin, and is not the `graphql-transport-ws` `next`
  frame (`{ id, type: "next", payload: { data } }`) that any real GraphQL subscription
  client emits. Provider verification is therefore testing a shape that doesn't exist on
  the wire.
- `subscription: options.operationName` puts an operation name in a field called
  `subscription`.
- The interaction description is hardcoded to `'a GraphQL subscription event'`
  (`index.ts:172`), so two subscription messages in one pact collide.
- The subscription case in `pact.test.ts:310-333` asserts only `toBeDefined()`.

### 1.5 Missing capability surface

No support for: fragments across documents, `extensions`, persisted/hashed queries,
batched operations, multipart file uploads, or `@defer`/`@stream`. `transport:
'query_string'` exists in the type (`types.ts:1`) and in the Rust parser, but there is no
GET helper in either binding, so it isn't reachable from the DSL.

---

## 2. Core across languages — Partially met (wrong seam)

The Rust core genuinely owns schema parsing (`schema.rs`), selection-set validation
(`graphql_payload.rs:596`), canonical body generation (`server.rs:396`), request diffing
(`graphql_payload.rs:175`), and provider verification (`server.rs:283`). That's the right
set of responsibilities and it's language-neutral. Good.

But the boundary leaks:

**Duplicated normalisation.** `normalizeQuery` is implemented three times — in Rust
(`graphql_payload.rs:1463`), TypeScript (`index.ts:12-40`), and Go (`graphql.go:73-114`) —
and the implementations already differ:

| Case | Rust | TS | Go |
|------|------|----|----|
| Lone `\r` as line ending | n/a | not handled | handled (`graphql.go:75`) |
| Line shorter than `minIndent` | char-wise skip | `slice(minIndent)` | emits `""` |

Since the plugin re-dedents everything it receives, the client-side pass is both redundant
*and* a source of cross-language divergence.

**Protocol decisions in bindings.** The message envelope (§1.4) is defined in JS, not in
the plugin. Go's `message.go` defines its own. There is no single source of truth.

**No conformance suite.** There is no shared fixture set that both bindings run against to
prove the TS and Go paths produce byte-identical plugin configuration.

---

## 3. Bindings do no heavy lifting — Not met

Both bindings currently do real, duplicable work:

| Function | JS | Go | Should live in |
|----------|----|----|----------------|
| `normalizeQuery` | `index.ts:12` | `graphql.go:73` | plugin (already there) |
| `serializeVariables` | `index.ts:42` | `graphql.go:116` | plugin |
| `buildEnvelopeVariables` | `index.ts:63` | `graphql.go:137` | plugin |
| `resolveTransport` | `index.ts:88` | `graphql.go:153` | plugin |
| `graphqlRequestBody` | `index.ts:79` | `graphql.go:44` | plugin — `generate_content` already produces exactly this |
| message envelope construction | `index.ts:152` | `message.go` | plugin |

`graphqlRequestBody` is the clearest example: the plugin implements `generate_content`
(`server.rs:84`) which rebuilds the canonical body from the stored configuration. The
bindings reimplement that rather than calling it, so the runtime request is generated by a
*different* code path than the expectation. That is the exact class of bug the plugin
architecture is supposed to eliminate.

---

## 4. Value beyond the raw Pact DSL — Marginal today

### What it genuinely adds

- Consumer-side query validation against SDL at pact-write time (when surfaced — §1.3).
- Whitespace/indentation-insensitive query comparison.
- Named, GraphQL-flavoured mismatch descriptions (`"GraphQL query document differs"`).

### What undercuts it

**4.1 The response is entirely GraphQL-unaware.**

**Update (2026-07-24, `docs/plans/2026-07-24-response-part-wiring.md`).** This is now only
partially true, and only in one direction. The plugin registers as a `ContentMatcher` for
`application/graphql-response` (`server.rs`) and `graphqlHttpInteraction`'s `response` option
routes the expected body through a response-side `configure_interaction` call. When the
response is well-formed, that call derives Pact matching rules from the schema — `match: type`
for scalars, `match: regex` over the enum's declared values for enum fields — and attaches them
to the response part instead of the plain-equality body Pact core would otherwise record. This
was confirmed by inspecting the generated pact file (the response body carries a `matchingRules`
block keyed by field path) and by running provider verification against the example's real
server, which still passes with these rules in place. So for **well-formed** responses, the
plugin is no longer response-unaware: type and enum mismatches introduced later (e.g. a
provider regression) would now be caught by core's JSON matcher using schema-derived rules that
didn't exist before this change.

What is **not** fixed, and must not be read as fixed by the above: the plugin still cannot make
a malformed *expected* response fail the consumer test. The response-side `configure_interaction`
call does correctly detect and reject (as an `Err`) a response containing a field absent from
the schema or absent from the query's selection set — this was directly observed in the
plugin's own log during the Task 1 spike — but that rejection never reaches the JS caller, for
the `pact-core` reason detailed in §1.3. So, unchanged from the original finding:

- fields in the response that aren't in the schema still silently make it into the pact file if
  the test author writes them into the expected `response` (the plugin rejects internally, but
  the author sees no failure and the interaction is recorded incomplete rather than correct),
- fields in the response that aren't in the query's selection set have the same gap,
- what *is* now caught, because it flows through core's matching rules rather than plugin-side
  configure-time validation, is a mismatch between a well-formed expected response and what a
  real provider actually returns (wrong scalar/enum type, wrong enum value) — this is the part
  of "response correctness" this task closes.

The distinction matters: this task delivers response *matching*, not response *authoring-time
rejection*. The latter needs the upstream `pact-core` fix described in §1.3 before the
"response not in schema" and "field not selected" example tests can be rewritten to assert on
the plugin instead of validating locally.

**4.2 Query comparison is string equality, not semantic.** `diff` compares
`query_document` as a `String` (`graphql_payload.rs:178`) after only dedent+trim. The AST is
already parsed two lines earlier for validation and then discarded. Consequences:

- `product(id: $id)` vs `product(id : $id)` → spurious mismatch.
- Reordered sibling fields → spurious mismatch (GraphQL response order follows the query, so
  this may be intentional, but it should be a documented *choice*, not an accident).
- A comment added to the query → spurious mismatch.
- A fragment inlined by a client codegen tool → spurious mismatch.
- The whole document is dumped into `expected`/`actual`, so the failure message is two
  20-line blobs rather than "field `category.name` missing at `products.category`".

**4.3 Variables lose matcher support — a regression vs raw Pact.** `variables_json` is
compared by exact string equality (`graphql_payload.rs:195-201`). With the raw DSL a user
can write `jsonBody({ query, variables: { id: like('10') } })` and get flexible matching.
With this plugin they cannot. Any non-deterministic variable (a UUID, a timestamp, a cursor)
makes the plugin unusable, and the escape hatch is to stop using it.

**Net:** a user can reproduce most of today's benefit with raw Pact plus a five-line local
`validate(buildSchema(sdl), parse(query))` call — which is precisely what
`examples/js/product-consumer/pact.test.ts` does.

---

## Recommendations

Ordered by value-per-unit-effort.

### P0 — Make the response schema-aware

Register a response-side content matcher and expose it in the DSL. On `compare_contents` for
the response, walk the *query's selection set* against the schema and the supplied body:

- reject fields absent from the schema, and fields absent from the selection set;
- enforce scalar/enum types, non-null-ness, and list-ness from the schema;
- understand the `{ data, errors, extensions }` envelope, including partial data with errors;
- **auto-derive matchers** from schema types where the user didn't supply one — `ID`/`String`
  → type matcher, `Int`/`Float` → type matcher, enum → `oneOf` of the enum's values, list →
  `eachLike`. This makes the common case both stricter *and* less typing than raw Pact,
  which is the definition of "adds value".

### P0 — Compare queries on the AST, not the string

Reuse the already-parsed document. Normalise (strip comments, canonical whitespace, inline
or canonically-order fragments, preserve aliases and argument values) and diff
node-by-node, reporting the specific path that differs. Expose a mode:

```
queryMatching: 'exact' | 'semantic' (default) | 'subset'
```

where `subset` permits the actual query to request a subset of the expected selection set —
useful when a codegen client trims unused fields.

### P0 — Surface plugin errors properly

Fix error propagation from `configure_interaction` through the Pact JS/Go drivers, and add a
fail-fast pre-check in the helpers so the user always gets a real GraphQL error message.
Then rewrite the "schema invalid" example tests to assert on the *plugin's* rejection.

### P1 — Matchers on variables

Accept Pact matchers inside `variables`, extract the matching rules, and send them to core
alongside the interaction so `{ id: like('10') }` works.

### P1 — Move all normalisation into the plugin

Delete `normalizeQuery`, `serializeVariables`, `buildEnvelopeVariables`, `resolveTransport`
and `graphqlRequestBody` from both bindings. Pass the raw query through. Have the bindings
obtain the runtime request body from the plugin's `generate_content` output (or from the
`ConfigureInteractionResponse` contents, which already contain it) so expectation and
runtime request share one code path.

Add `spec/conformance/*.json` fixtures — input config → expected canonical config → expected
generated body — and run them from Rust, TS and Go test suites.

### P1 — Bind the request emitter to the interaction

Return an executable client from the builder so the trio can't be written twice (see
examples below).

### P2 — Fix subscriptions

Define the envelope in the plugin. Support `graphql-transport-ws` framing as the default
with an option for a bare `{data}` payload. Allow a per-message description. Validate the
`data` payload against the subscription's selection set.

### P2 — Ergonomics

- Resolve plugin version from the helper package's own metadata; drop the env var from docs
  and examples.
- Configurable path/method; a working `query_string` (GET) transport in both DSLs.
- Accept a schema as SDL string, file path, or introspection JSON; parse and cache once.
- Ship a `gql` tagged template so editors provide GraphQL syntax highlighting and LSP
  support inside the test file.

---

## Updated examples

These illustrate the target DSL after the recommendations above. Everything the user writes
is GraphQL or an expectation; nothing is plumbing.

### TypeScript — query with schema-derived response matching

```ts
import { PactV4 } from '@pact-foundation/pact';
import { like } from '@pact-foundation/pact/dsl/matchers';
import { graphql, gql } from '@pact-foundation/pact-graphql-helper';

const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
// Schema is parsed and cached once; accepts SDL, a file path, or an introspection result.
const api = graphql(pact, { schema: './schema.graphql', path: '/graphql' });

it('fetches a product', async () => {
  await api
    .interaction('a GraphQL product request')
    .given('a product with ID 10 exists')
    .query(gql`
      query GetProduct($id: ID!) {
        product(id: $id) {
          id
          name
          status
        }
      }
    `)
    .variables({ id: like('10') })          // matchers work on variables
    .willRespondWith({
      data: {
        product: {
          id: '10',
          name: like('product name'),
          status: 'ACTIVE',                 // validated against the ProductStatus enum
        },
      },
    })
    .executeTest(async (client) => {
      // `client` is pre-wired to the mock server, the right path, method and content type,
      // and replays the exact canonical request the plugin recorded.
      const { data } = await client.execute();
      expect(data.product.id).toBe('10');
    });
});
```

Notes on what changed:
- The query, variables and operation name are written once. `client.execute()` replays the
  plugin-generated body, so consumer and expectation cannot drift.
- `operationName` is inferred from the document; still overridable for multi-operation docs.
- `.willRespondWith` takes the GraphQL envelope. `name` gets an explicit matcher; `id` and
  `status` get matchers derived from the schema (`ID` and the enum) automatically.
- No content type, no env var, no `graphqlRequestBody`.

### TypeScript — an invalid query now actually fails

```ts
it('rejects a field that is not in the schema', async () => {
  await expect(
    api.interaction('an invalid query').query(gql`
      query InvalidField($id: ID!) {
        product(id: $id) { id stockLevel }
      }
    `).build(),
  ).rejects.toThrow(/Cannot query field "stockLevel" on type "Product"/);
});

it('rejects a response field that is not in the schema', async () => {
  await expect(
    api.interaction('a response with an unknown field')
      .query(gql`query GetProduct($id: ID!) { product(id: $id) { id name } }`)
      .variables({ id: '10' })
      .willRespondWith({
        data: { product: { id: '10', name: 'product name', internalSku: 'INT-001' } },
      })
      .build(),
  ).rejects.toThrow(/"internalSku" is not a field of type "Product"/);
});

it('rejects a response field that was not selected', async () => {
  await expect(
    api.interaction('a response with an unselected field')
      .query(gql`query GetProduct($id: ID!) { product(id: $id) { id } }`)
      .variables({ id: '10' })
      .willRespondWith({ data: { product: { id: '10', name: 'product name' } } })
      .build(),
  ).rejects.toThrow(/"name" was not requested by operation "GetProduct"/);
});
```

These are the tests that today live outside the plugin as local `validate()` calls.

### TypeScript — GraphQL error envelope

```ts
await api
  .interaction('a product that does not exist')
  .given('no product with ID 999 exists')
  .query(gql`query GetProduct($id: ID!) { product(id: $id) { id name } }`)
  .variables({ id: '999' })
  .willRespondWith({
    data: { product: null },                 // nullable in the schema — accepted
    errors: [
      {
        message: like('Product not found'),
        path: ['product'],
        extensions: { code: 'NOT_FOUND' },
      },
    ],
  })
  .executeTest(async (client) => {
    const res = await client.execute();
    expect(res.errors?.[0].extensions.code).toBe('NOT_FOUND');
  });
```

### TypeScript — subscription over graphql-transport-ws

```ts
await api
  .message('an inventory level change for var-1')
  .given('inventory updates are available')
  .subscription(gql`
    subscription InventoryChanged($variantId: ID!) {
      inventoryChanged(variantId: $variantId) { quantity updatedAt }
    }
  `)
  .variables({ variantId: 'var-1' })
  .withProtocol('graphql-transport-ws')      // default; 'raw' gives a bare { data } payload
  .willReceive({
    data: {
      inventoryChanged: { quantity: like(42), updatedAt: like('2026-03-08T12:00:00Z') },
    },
  })
  .executeTest(async (frame) => {
    expect(frame.type).toBe('next');
    expect(frame.payload.data.inventoryChanged.quantity).toBe(42);
  });
```

Per-message description, real wire framing, and `data` validated against the subscription's
selection set.

### Go — equivalent

```go
api, err := pactgraphql.New(pact, pactgraphql.Config{
    SchemaFile: "schema.graphql",
    Path:       "/graphql",
})
if err != nil {
    t.Fatal(err)
}

err = api.
    Interaction("a GraphQL product request").
    Given("a product with ID 10 exists").
    Query(`
      query GetProduct($id: ID!) {
        product(id: $id) { id name status }
      }
    `).
    Variables(map[string]any{"id": matchers.Like("10")}).
    WillRespondWith(pactgraphql.Response{
        Data: map[string]any{
            "product": map[string]any{
                "id":     "10",
                "name":   matchers.Like("product name"),
                "status": "ACTIVE",
            },
        },
    }).
    ExecuteTest(t, func(client pactgraphql.Client) error {
        res, err := client.Execute()
        if err != nil {
            return err
        }
        if res.Data["product"].(map[string]any)["id"] != "10" {
            return fmt.Errorf("unexpected product id")
        }
        return nil
    })
if err != nil {
    t.Fatal(err)
}
```

No `GraphQLRequestBody`, no manual `http.NewRequest`, no `content-type` header, no
`t.Setenv("PACT_GRAPHQL_PLUGIN_VERSION", ...)`. The Go binding is now a thin fluent wrapper
over the same plugin configuration the TS binding produces — which is objective 3.

### Go — subset query matching for a codegen client

```go
api.Interaction("a trimmed product query").
    Query(canonicalQuery).
    QueryMatching(pactgraphql.QueryMatchingSubset).  // client may request fewer fields
    WillRespondWith(...)
```

---

## Suggested sequencing

1. Response-side matching + schema-derived matchers (P0) — unlocks the core value prop.
2. AST-based query diff with field-level mismatch paths (P0) — removes false failures.
3. Error propagation fix + rewrite the negative examples to exercise the plugin (P0).
4. Variable matchers (P1).
5. Strip the bindings back to thin wrappers; add the conformance fixture suite (P1).
6. Fluent builder + bound client in TS and Go (P1).
7. Subscription protocol, ergonomics, version resolution (P2).

Steps 1–3 are what move objective 4 from "marginal" to "clearly worth adopting". Steps 5–6
are what move objectives 1 and 3 from "not met" to "met".
