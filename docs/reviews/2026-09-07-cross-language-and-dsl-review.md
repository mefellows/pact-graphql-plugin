# GraphQL Plugin — Cross-Language Core & JS/TS DSL Review

Date: 2026-09-07
Scope: `pact-graphql-plugin/` (Rust core), `js/pact-graphql-helper/`, `go/pactgraphql/`,
`examples/{js,go}/product-consumer/`, `pact-plugin.json`
Lens: **SDK-agnostic core, first-class JS/TS DSL**

Follows `docs/reviews/2026-07-22-objectives-review.md`. That review's findings are not repeated
here except where the current state has moved; §1 records the delta, §2 is new material.

---

## 0. Summary

The Rust core has advanced materially since July: query comparison is now AST-based with three
matching modes, and the response part is schema-validated with auto-derived matching rules that
demonstrably land in the pact file. Both of those were P0s and both are real.

The problem now is almost entirely at the **seam**, and it has a specific shape:

> The core knows more than any binding can ask it, and the pact file records more than it needs to.

Three concrete symptoms, all verified in this pass:

1. `query_matching` (`exact` / `semantic` / `subset`) and the `query_string` transport are
   implemented and unit-tested in Rust, and **unreachable from both the TS and Go DSLs**.
2. The response part — the plugin's headline value — is reachable from TS only, and not at all
   from Go. The two bindings have forked, not diverged.
3. 80% of the example pact file (34,996 of 43,660 bytes) is plugin configuration, and most of
   that is the same SDL base64-encoded **eight times** — twice per interaction, across four
   interactions.

And one latent defect that the test suite cannot catch because it tests a fixture that does not
match reality: `config_from_pact` reads a pact shape `pact_models` never writes (§2.1).

---

## 1. Delta since 2026-07-22

| Prior finding | Status |
|---|---|
| §4.2 Query comparison is string equality | **Fixed.** `query_ast::parse_and_inline` + `diff_operations_with`; fragments inlined, comments and whitespace ignored, field-level mismatch paths. |
| §4.1 Response is GraphQL-unaware | **Fixed for matching.** `configure_response` validates against schema + selection set and derives `match: type` / enum `match: regex` rules. Verified in the generated pact. |
| §1.3 Plugin errors don't surface | **Open.** Still blocked on `pact-core`'s `withPluginResponseInteractionContents` hardcoding `return true`. Examples still validate locally (`pact.test.ts:23`, `:475`). |
| §4.3 Variables lose matcher support | **Open.** `variables_json` is still compared by string equality. |
| §3 Bindings do heavy lifting | **Open and now worse.** `normalizeQuery` / `serializeVariables` / `resolveTransport` / `graphqlRequestBody` still duplicated in both bindings; the two bindings have additionally diverged in *capability*. |
| §1.4 Subscriptions half-built | **Open.** Envelope still invented in JS (`index.ts:152`), description still hardcoded (`index.ts:172`). |
| §1.1 Query trio written twice | **Open.** `examples/js/product-consumer/pact.test.ts:49-73`. |

Net: the two hardest core problems are solved. Everything remaining is boundary and packaging
work, which is good news — it is the cheaper half.

---

## 2. New findings

### 2.1 `config_from_pact` reads a pact shape that is never written — **defect, high**

`server.rs:772-781` requires:

```
interaction.pluginConfiguration.graphql.interactionConfiguration
```

`pact_models` serializes `plugin_config` as `HashMap<String, HashMap<String, Value>>`
(`pact_models-1.3.14/src/v4/synch_http.rs:165`) — i.e. the config sits **flat** under the plugin
name, with no `interactionConfiguration` wrapper. The example's own pact file confirms it:

```
pluginConfiguration.graphql keys:
  ['inline_schema', 'operation_name', 'query_document', 'query_matching',
   'request', 'response_body_json', 'schema_inline_base64', 'schema_ref',
   'transport', 'variables_json']
```

So `prepare_interaction_for_verification` and `verify_interaction` would both fail with
`plugin configuration for 'graphql' not found in pact interaction` against any real pact.

This is invisible today for two reasons, and both should be fixed:

- The only test fixture, `pact_with_config` (`server.rs:1358-1372`), *constructs* the
  `interactionConfiguration` nesting by hand. The tests therefore prove the code is
  self-consistent, not that it is correct.
- `examples/js/product-consumer/verify-provider.test.ts` passes because core generates the
  request body from the recorded body and matches the response with its own JSON matcher —
  by design (`docs/plans/2026-07-24-response-part-wiring.md`) — so neither hook is invoked.

**Fix:** accept both shapes (flat, and nested for forward compatibility), and replace the
hand-built fixture with the real pact file from `examples/js/product-consumer/pacts/` committed
as a test fixture. This is the single highest-value test change in the repo: it converts the
verification tests from tautology to evidence.

### 2.2 Variable key order causes spurious mismatches — **defect, medium**

`canonicalize_variables` (`graphql_payload.rs:866`) round-trips through `serde_json` on the
assumption that the default `Map` is a sorted `BTreeMap`. But `Cargo.toml:9` enables
`serde_json`'s `preserve_order` feature, which swaps in `IndexMap`. Key order is therefore
preserved, not canonicalised, and `variables_json` is compared as an exact string
(`graphql_payload.rs:251-257`).

Consequence: `{"a":1,"b":2}` and `{"b":2,"a":1}` are the same GraphQL variables and a Pact
mismatch. Any client that builds variables from a map with non-deterministic iteration order
(Go's `map[string]any` most obviously) will flake.

**Fix:** canonicalise explicitly — recursively sort object keys before serialising — rather than
relying on a `serde_json` feature flag that another dependency could flip. Add a test that the
two orderings compare equal.

### 2.3 The pact file carries the schema eight times

Measured on `examples/js/product-consumer/pacts/product-consumer-product-provider.json`:

| | bytes |
|---|---|
| Whole pact | 43,660 |
| `pluginConfiguration` across 4 interactions | 34,996 |
| SDL base64, per copy | 3,816 |
| Copies | 8 (2 per interaction × 4) |

Two independent redundancies:

1. **Within an interaction:** `schema_inline_base64` and `inline_schema.base64_sdl` hold byte-identical
   values. The wire struct populates each from the other (`interaction.rs:47-52`, `:113-124`) to
   support a legacy field that is no longer written by anything. Similarly, `request` duplicates
   the sibling `query_document` / `operation_name` / `variables_json` / `transport` fields.
2. **Across interactions:** every interaction embeds the whole schema. `PluginConfiguration` has a
   `pact_configuration` field for exactly this — a per-pact, plugin-scoped slot — and the plugin
   sets it to `None` (`server.rs:87`). The pact's `metadata.plugins[0].configuration` is `{}`.

For a realistic schema (a few hundred KB is normal for a federated graph) and a few dozen
interactions, pacts become tens of megabytes of duplicated base64 pushed to the broker on every
CI run.

**Fix:** move the SDL into `pact_configuration` keyed by content hash; keep only `schema_ref`
(the hash) on the interaction. `SchemaRef` already exists and is already written — it just has
nothing to resolve against yet (§2.4). Drop `schema_inline_base64` on write, keep reading it for
one release. Drop the `request` sub-object in favour of the flat fields, or vice versa — but not
both.

### 2.4 `SchemaRegistry` is a filesystem side effect that cannot work cross-machine

`GraphqlInteractionBuilder::build` calls `registry.store(sdl)` on every `configure_interaction`
(`interaction.rs:190`), writing `.pact-graphql-plugin/schemas/<sha256>.graphql` relative to
whatever `cwd` the plugin process happened to inherit (`server.rs:812-817`). Meanwhile
`CanonicalGraphqlRequest::from_interaction_config` takes the registry as `_registry` and ignores
it (`graphql_payload.rs:70-72`).

So the registry: writes files nobody reads on the consumer side; produces a `schema_ref` hash
that `resolve_schema_sdl_indicator` (`graphql_payload.rs:408-421`) will try to resolve from a
local directory that, on the provider's machine, does not contain it; and creates a stray
directory in the user's repo.

**Fix:** decide what it is. Either (a) delete it and rely on the inline SDL, or (b) — better, and
it composes with §2.3 — make it a genuine content-addressed cache whose miss path falls back to
the SDL carried in `pact_configuration`. Right now it is neither, and `_registry` is the tell.

### 2.5 Core capabilities are marooned behind the bindings

Implemented in Rust, unreachable from any DSL:

| Capability | Core | TS | Go |
|---|---|---|---|
| `query_matching: exact \| semantic \| subset` | `query_ast.rs:387` | ✗ | ✗ |
| `transport: query_string` (GET) | `encoder.rs` | typed but no GET helper | typed but no GET helper |
| Response validation + derived rules | `server.rs:113` | ✓ | ✗ |
| Configurable path / method | n/a | hardcoded `/graphql` (`index.ts:130`) | hardcoded (`http_interaction.go:31`) |

`buildGraphqlConfiguration` (`index.ts:91-102`) and `buildGraphQLConfiguration`
(`graphql.go:160`) each hand-write the wire struct and each omit different fields. There is no
type shared with the Rust `GraphqlPluginRequest`, so every new core field requires two manual
edits that nothing verifies.

**Fix:** generate the wire types. `GraphqlPluginRequest` is the contract — emit a JSON Schema from
it in `build.rs` and generate `types.ts` and the Go struct from that schema in CI. Then a field
added to the core is a compile error in both bindings, not a silent omission.

### 2.6 Go is a fork, not a binding

`go/pactgraphql/graphql.go` reimplements `normalizeQuery`, `serializeVariables`,
`buildEnvelopeVariables` and `resolveTransport`, and the implementations still differ from the TS
ones in the ways the July review tabulated (lone `\r`, short-line handling). Since the plugin
re-canonicalises everything it receives, this work is not only duplicated but *load-bearing for
nothing* — except that when the two disagree, the pact differs by language.

**Fix:** pass the raw query straight through. Delete all four functions from both bindings. Have
the bindings obtain the runtime request body from the plugin's `generate_content` output (or from
the contents already returned by `configure_interaction`) so the expectation and the request the
test actually sends come from one code path.

### 2.7 Nothing validates variables against the schema

`validate_query_document` → `validate_operation` (`graphql_payload.rs:535`) checks the selection
set only. Not checked, despite the schema being in hand:

- field **arguments** against their declared types,
- the operation's **variable definitions** (`$id: ID!`) against the supplied `variables_json` —
  missing required variables, unknown variables, wrong types,
- input object shapes for mutations.

This is the cheapest remaining "why is this better than raw Pact" win: a consumer who typos
`{ productId: '10' }` for `$id: ID!` currently writes a pact that a real server would reject.

### 2.8 Manifest and runtime catalogue disagree

`pact-plugin.json` declares one content type, `application/graphql`, for both capabilities.
`init_plugin` (`server.rs:196-229`) additionally registers a `graphql-response` content matcher
for `application/graphql-response`. Whatever the driver actually keys off, the two should not
drift — a reader has no way to know which is authoritative.

### 2.9 No CI, and no conformance suite

There is no `.github/`. A three-language repository — Rust core, TS binding, Go binding, plus two
example projects that are the only end-to-end proof anything works — has no automated build, no
cross-language check, and no release verification. The July review's `spec/conformance/*.json`
recommendation is unimplemented and is the natural thing to build CI around.

### 2.10 Packaging and version inconsistencies

- README installs `@pact-foundation/pact-graphql-helper`; `package.json` names the package
  `pact-graphql-helper` at version `1.0.0`; the example depends on it via `file:`.
- Plugin version defaults disagree: helper `'0.0.0'` (`index.ts:110`), Go `"0.0.0"`
  (`http_interaction.go:25`), `Cargo.toml` `0.1.0`, `pact-plugin.json` `0.0.0`, examples set
  `PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0'`.
- `PACT_GRAPHQL_PLUGIN_VERSION` appears at the top of every example file. A consumer author
  should never see this — resolve it from the helper package's own metadata.

---

## 3. The shape to aim for

One sentence per layer:

**Core (Rust)** owns everything GraphQL: parsing, canonicalisation, validation (query *and*
variables *and* response), diffing, matcher derivation, and the request body that goes on the
wire. It is the only place a GraphQL fact is known.

**Wire contract** is a versioned, generated schema derived from `GraphqlPluginRequest`. Bindings
do not hand-write it.

**Bindings** are fluent sugar over that struct plus the Pact DSL, and nothing else. A binding
should contain no `normalize`, no `serialize`, no `parse`, no envelope construction. The test for
"is this binding too thick" is: *could I delete this function and lose nothing but keystrokes?*

**Runtime request** comes from the plugin, not from the binding. The single most valuable
structural change in the DSL is that the user cannot write the query twice, because the client
handed to `executeTest` replays what the plugin recorded.

### Target TS DSL

```ts
import { PactV4 } from '@pact-foundation/pact';
import { like } from '@pact-foundation/pact/dsl/matchers';
import { graphql, gql } from '@pact-foundation/pact-graphql-helper';

const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });

// Schema read and hashed once; SDL string, file path, or introspection JSON.
// Path, method and transport are stated here, not baked into the helper.
const api = graphql(pact, { schema: './schema.graphql', path: '/graphql' });

it('fetches a product', async () => {
  await api
    .interaction('a GraphQL product request')
    .given('a product with ID 10 exists')
    .query(gql`
      query GetProduct($id: ID!) {
        product(id: $id) { id name status }
      }
    `)
    .variables({ id: like('10') })
    .matching('semantic')                    // exact | semantic (default) | subset
    .willRespondWith({
      data: { product: { id: '10', name: like('product name'), status: 'ACTIVE' } },
    })
    .executeTest(async (client) => {
      const { data } = await client.execute();   // replays the plugin's canonical body
      expect(data.product.id).toBe('10');
    });
});
```

What each piece buys, mapped to a finding:

- `graphql(pact, { schema })` — schema parsed and hashed once per pact, not per interaction (§2.3);
  configurable path (§2.5); no env var (§2.10).
- `gql` tagged template — editor highlighting and LSP inside the test; it is *not* a parser, it
  returns the raw string.
- `.matching(...)` — exposes the core mode that exists today and nobody can reach (§2.5).
- `.variables(...)` accepting matchers — the July §4.3 regression; the binding extracts the rules
  and hands them to core, it does not interpret them.
- `client.execute()` — kills the write-it-twice bug (July §1.1) by construction.
- No `content-type`, no `graphqlRequestBody`, no `PACT_GRAPHQL_PLUGIN_VERSION`.

The Go DSL should be the same object model with Go naming, generated from the same wire schema —
so that adding `.matching()` is one core change plus two generated structs, not two hand-edits.

### On surfacing authoring-time errors

The blocker is upstream: `@pact-foundation/pact-core`'s `withPluginResponseInteractionContents`
hardcodes `return true` and discards the FFI error code, so a plugin rejection never reaches the
JS caller. Two moves, in this order:

1. **Fix it upstream.** It is a small, well-scoped PR against `pact-core` and it is the only fix
   that works for every language driver at once. Nothing else in this document unblocks as much
   for as little.
2. **Interim shim only, clearly labelled.** Until that lands, an optional `graphql` peer dependency
   in the TS helper can run `validate(buildSchema(sdl), parse(query))` as a fail-fast so the author
   at least gets a real GraphQL error. Accept this as a knowing, temporary duplicate of core logic
   — and delete it the day the upstream fix ships. Do not extend the shim to response validation;
   that would re-import the whole selection-set walker into the binding, which is exactly the
   boundary this review is arguing against.

Until (1) lands, the "invalid query" and "response not in schema" examples remain local
`validate()` calls, and they should carry a comment saying so — they are currently the strongest
evidence a reader has that the plugin does not do what it claims.

---

## 4. Prioritised roadmap

Ordered by value per unit effort. P0s are correctness or credibility; P1s are the objectives.

**P0**

1. **§2.1** Accept the real pact shape in `config_from_pact`; replace the hand-built fixture with a
   committed real pact file. *Small change, converts the verification test suite from tautology to
   evidence.*
2. **§2.2** Canonicalise variable key order explicitly; test both orderings compare equal.
3. **§3** Upstream `pact-core` fix for plugin error propagation, plus the labelled interim shim.
   *Everything about the plugin's perceived value is gated on this.*

**P1**

4. **§2.3 + §2.4** Move the SDL to `pact_configuration` keyed by hash; make `SchemaRegistry` a real
   cache or delete it; drop the duplicated `schema_inline_base64` and `request` fields.
5. **§2.5** Generate the wire types from `GraphqlPluginRequest`; expose `queryMatching`, `path`,
   `method` and `query_string` in both bindings.
6. **§2.6** Strip `normalizeQuery` / `serializeVariables` / `buildEnvelopeVariables` /
   `resolveTransport` / `graphqlRequestBody` from TS and Go; source the runtime body from the
   plugin.
7. **§3** Fluent TS builder with a bound client; Go follows from the same generated contract.
8. **§2.7** Validate arguments and variable definitions against the schema.

**P2**

9. **§2.9** CI: build the Rust core, run all three test suites, run the examples end to end, and
   run a `spec/conformance/*.json` fixture set from Rust, TS and Go so the bindings are proven
   byte-identical.
10. **§2.10 / §2.8** Version resolution from package metadata; reconcile `pact-plugin.json` with
    the runtime catalogue; fix the package name.
11. Subscriptions: define the envelope in the plugin, support `graphql-transport-ws` framing,
    per-message descriptions, and validate `data` against the subscription's selection set
    (July §1.4).

Items 1–3 protect what has already been built. Items 4–8 are what move "core works across
languages" and "bindings do no heavy lifting" from partially met to met — and item 6 is the one
that makes the JS DSL genuinely first-class rather than merely convenient.


---

## Addendum — implementation status (2026-09-07)

Batches P0 and the core of P1 have been implemented in this session. Test counts: Rust 80 → 112,
JS helper 10 → 25, JS example 8 → 15 (the example suite now also runs provider verification in a
single `vitest run`, which previously needed manual ordering).

### Landed

| # | Item | Evidence |
|---|------|----------|
| P0.1 | `config_from_pact` accepts the flat shape `pact_models` writes, and the hand-built fixture is replaced by a real pact file (`pact-graphql-plugin/tests/fixtures/pact-ffi-written.json`) captured from the example | 3 new tests; the flat-shape one failed with the exact predicted error before the fix |
| P0.2 | Variable key order canonicalised recursively (`sort_object_keys`); array order deliberately preserved | 3 new tests |
| P1.4a | SDL written once per interaction instead of twice; absent fields omitted rather than serialised as `null` | Plugin configuration for an identical interaction: **8,304 → 4,432 bytes (−47%)** |
| P1.8 | Variable definitions validated against supplied variables — presence, nullability, scalar coercion, enums, lists, input objects | New `src/variables.rs`, 24 tests |
| P1.5/6/7 | New fluent TS DSL: `graphql()` / `gql` / `.query().variables().matching().willRespondWith().executeTest(client)`, with a bound client that replays the declared operation | `src/dsl.ts`, 15 tests, plus 4 end-to-end example tests against the real plugin |
| — | gRPC errors now carry the `anyhow` cause chain (`{err:#}`) instead of only the outermost context | 1 test; this is what made the regression below diagnosable |

### Bugs found while implementing

Three defects surfaced that were not in the original review, all now fixed:

1. **`just install` passed `target=<triple>` as a positional argument** to its own recipes, so
   `just install`, `just bundle-all` and the README's documented `just bundle target=<triple>` all
   failed with `unsupported target 'target=aarch64-apple-darwin'`. Only the bare positional form
   worked. Both spellings are now accepted.
2. **`just install` copied over the installed binary in place.** On macOS this invalidates the code
   signature and the kernel SIGKILLs the process, so the plugin failed to start with
   `Plugin process did not output the correct startup message`. It now removes the old binary first
   and re-signs ad-hoc where `codesign` is available.
3. **`just install` only built when the bundle archive was missing**, so it silently installed a
   stale binary after any source change. This cost real debugging time in this session — a fix
   appeared not to work when in fact it had never been installed. It now always rebuilds.

Plus one in the example suite: **vitest runs test files in parallel, and concurrent writers clobber
each other's interactions in the shared pact file.** Adding a second consumer test file made the
generated pact silently lose an interaction that was present when the files ran individually. Fixed
with `fileParallelism: false` in a new `examples/js/product-consumer/vitest.config.ts`.

### Regression caught by the new validation

Adding variable validation initially broke every interaction with an expected response. The
response-side `configure_interaction` call is a separate invocation and was not carrying
`variables_json`, so the plugin saw an operation whose declared `$id: ID!` was unsatisfied and
rejected it — and, because of the upstream `pact-core` bug below, the rejection surfaced only as a
silently bodyless response part. Both the DSL and the legacy `graphqlHttpInteraction` now carry the
variables on the response-side call. This is worth recording because it is the same class of bug
§2.1 describes: a separate call that must restate the whole operation, with no type forcing it to.

### Still open

- **P0.3 — the upstream `pact-core` fix remains the single highest-value item.** It is now
  confirmed by direct observation twice over: the plugin correctly rejects both an invalid
  selection set and unsatisfiable variables, and in both cases the JS caller sees success. Two
  `it.fails` tests in `dsl.test.ts` pin the broken behaviour and will flip when it is fixed. Until
  then the plugin's authoring-time validation is invisible to consumers, which undercuts most of
  its value.
- §2.3 cross-interaction SDL dedup via `pact_configuration` (only the intra-interaction duplicate
  was removed; the schema is still embedded once per interaction).
- §2.4 `SchemaRegistry` still writes cwd-relative files nobody reads — visible as a stray
  `.pact-graphql-plugin/` directory created inside `~/.pact/plugins/graphql-0.1.0/` during this
  session.
- §2.5 generated wire types; §2.6 stripping the Go binding; §2.9 CI; §2.10 packaging.
- The Go binding is unchanged and still cannot reach the response part, `queryMatching`, or a
  configurable path.


---

## Addendum 2 — after pact-js-core 20.1.1 (2026-09-07)

`pact-foundation/pact-js-core#956` shipped. Upgrading the example and helper to
`@pact-foundation/pact` 17.1.4 (`pact-core` 20.1.1) closes **P0.3**, the item this review called
the single highest-value fix.

The upstream bug was deeper than §1.3 described. It was three layers, not one: `native/consumer.cc`
coerced the FFI status to a `bool` — mapping 0 (success) to `false` and 6 (plugin error) to `true`,
exactly backwards — `src/ffi/types.ts` declared the function `void`, and only then did
`src/consumer/index.ts` hardcode `return true` at ten call sites.

### Verified end to end

Both rejection tests in `examples/js/product-consumer/dsl.test.ts` now assert on the plugin and
pass. The message a consumer sees:

```
Failed to set plugin interaction contents for content type 'application/graphql':
the plugin returned an error: Failed to call out to plugin - Call to plugin failed -
code: 'Client specified an invalid argument', message: "GraphQL query validation failed:
field `stockLevel` does not exist on type `Product`"
```

The tail of that message is the `to_status` cause-chain change from Addendum 1. Without it the
message would stop at "GraphQL query validation failed" — `pact_ffi` calls
`set_error_msg(err.to_string())` (`pact_ffi/src/plugins/mod.rs:283`), which prints only the
outermost `anyhow` context, so a plugin that buries its reason in a cause loses it.

**Note for plugin authors:** validation fires when the plugin *contents* are set
(`pactffi_interaction_contents`), not when the plugin is loaded (`usingPlugin`). Tests asserting on
rejection must go through a call that sets contents.

### The example's local-validation tests are gone

The review's most serious DX finding (§1.3, "the two headline features do not actually fail a
test") is closed. `pact.test.ts` no longer calls `graphql`'s own `validate()` anywhere:

- `rejects unknown field selections` — asserts the plugin's rejection.
- `rejects invalid enum values` — asserts the plugin's rejection (needed new argument validation,
  below).
- `rejects responses with unknown fields` — routed through the `response` option, so the plugin
  rejects it instead of a local check that previously wrote an invalid pact anyway.

The local `validateQuery` / `getUnknownFields` helpers and the `graphql` package imports that
supported them have been deleted.

### §2.7 completed — argument validation

`rejects invalid enum values` uses `products(status: DISCONTINUED)`, an enum literal in an
*argument*. The plugin accepted it: `validate_field_arguments` checked argument *names* and
required-ness but never argument *values*. Now added, mirroring the variable coercion rules over
the query AST (enums, built-in scalars, lists, input objects, non-null), with `$variable`
references deferred to the variable check. 8 new tests. §2.7 is closed apart from input-object
interiors, which still need the schema index to carry input object fields.

### Two more defects found

1. **`PACT_PLUGIN_HOST` was being used as a bind address.** It is the driver's *own* PluginHost
   log-forwarding server (`pact-plugins/drivers/rust/driver/src/grpc_plugin.rs`:
   `env("PACT_PLUGIN_HOST", format!("127.0.0.1:{}", port))`) — an address to connect to, never to
   bind to. The plugin had always misused it, and got away with it only because older drivers left
   it unset. The newer driver sets it, so the plugin first tried to bind `127.0.0.1:64741:0`
   (unresolvable), then, once that was "fixed", bound the driver's own port and died with "Address
   already in use". Either way the driver reported only `Plugin process did not output the correct
   startup message in 60 seconds`. Binding now defaults to `[::1]:0` and is overridable via
   `PACT_GRAPHQL_PLUGIN_BIND`; a regression test asserts `PACT_PLUGIN_HOST` is ignored.
2. **A plugin that failed to start produced no diagnostics.** `main` returned the error to stderr,
   which the driver discards, so the only signal was the 60-second timeout above. Startup failures
   and the working directory are now written to the plugin's own log — this is what made the
   diagnosis above possible, and it took two rounds without it.

Plus, in the example: provider verification consumes the pacts the consumer tests produce, and
vitest does not order files alphabetically, so verification raced ahead of the test generating its
input. The two phases are now explicitly separated — `npm test` writes pacts, `npm run verify`
verifies them, `npm run test:all` does both.

### Status

Rust 125 tests, JS helper 25, JS example 13 + 2 verification — all green from a clean slate.

Remaining from §2: cross-interaction SDL dedup via `pact_configuration` (§2.3), the
`SchemaRegistry` decision (§2.4), generated wire types (§2.5), stripping the Go binding (§2.6), CI
(§2.9), packaging (§2.10). The Go binding is still unchanged and cannot reach the response part,
`queryMatching`, or a configurable path — and it now also lags on everything above.

One new item: the legacy helper's structural types no longer match pact 17's builder types
(`V4RequestBuilder` has no `pluginContents` in its public type), so `pact.test.ts` does not
typecheck even though it runs. The new DSL is unaffected. This strengthens the §2.5 case for
generating the binding types from `GraphqlPluginRequest` rather than hand-writing them.


---

## Addendum 3 — the V4RequestBuilder typecheck failure (2026-09-07)

The diagnosis in Addendum 2 ("`V4RequestBuilder` has no `pluginContents` in its public type") was
right about the symptom and wrong about the cause. pact-js types plugins fully. The problem was
entirely ours.

### What pact-js actually models

A state machine, in which `pluginContents` appears only *after* `usingPlugin`:

```
V4UnconfiguredInteraction --usingPlugin()--> V4InteractionWithPlugin
    .withRequest(m, p, V4PluginRequestBuilderFunc)  -> V4InteractionWithPluginRequest
        builder: V4RequestWithPluginBuilder   -- has pluginContents
    .willRespondWith(s, V4PluginResponseBuilderFunc) -> V4InteractionWithPluginResponse
        builder: V4ResponseWithPluginBuilder  -- has pluginContents
    .executeTest(...)
```

`V4RequestBuilder` is the *non-plugin* branch, so its lack of `pluginContents` is correct. Our
helper hand-wrote a parallel set of structural types that collapsed these states and demanded
`pluginContents` on the base builder — a shape no pact-js type has, which is why all ten call sites
failed.

### Three bugs the wrong types were hiding

1. **`usingPlugin` ignores `configuration`.** pact-js's `PluginConfig` is `{plugin, version}`, and
   `UnconfiguredInteraction.usingPlugin` calls `addPlugin(config.plugin, config.version)` and
   nothing else. The helper passed a fully-built `configuration` object that was silently dropped.
   This is the root cause of the Addendum 2 note that "validation fires on `pluginContents`, not
   `usingPlugin`" — not a quirk of where validation runs, but a parameter that never went anywhere.
   `graphqlInteraction` was therefore an API that appeared to configure the plugin and did not; it
   is now typed honestly, documented, and deprecated in favour of the DSL.
2. **`usingPlugin` is synchronous.** It returns `V4InteractionWithPlugin`, not a promise. The helper
   awaited it and both test fakes modelled it as async, so the wrong mental model was baked into
   the tests as well as the code.
3. **`graphqlHttpInteraction` declared the wrong return type.** It returned
   `V4InteractionWithPluginResponse` even when no `response` option was supplied, so callers doing
   their own `.willRespondWith(...)` were calling a method the declared type does not have. It now
   has two overloads: with `response`, `...WithPluginResponse`; without, `...WithPluginRequest`.

Tests asserting the old contract have been rewritten to assert the real one — notably the
`graphqlInteraction` tests, which checked properties of a `configuration` object pact-js discarded
and so proved nothing.

### Why the types are still structural

`V4UnconfiguredInteraction` and friends live in `@pact-foundation/pact/src/v4/http/types` and are
**not re-exported from the package root** — `src/v4/index.d.ts` re-exports `./graphql`,
`./message/index` and `../xml`, but not `./http/types`. Importing them means reaching into
pact-js's `src/` layout, which is not a stable public surface for a helper with a peer-dependency
range.

So the types remain structural, but now mirror the real chain faithfully, including the full
builder member set (`jsonBody`, `body`, `matchingRules`, …) rather than only the members this
helper happens to call. Each carries a comment naming the pact-js type it mirrors.

**Worth raising upstream:** re-exporting the V4 plugin interaction types from the package root
would let plugin authors depend on them directly. The absence of that export is why this helper
hand-wrote types at all, and hand-written types are how three behavioural bugs went unnoticed.

### Root cause of the drift: nothing typechecked

The example had `tsconfig.json` scoped to `include: ["pact.test.ts"]` and no `typecheck` script,
and vitest transpiles without typechecking — so the example could not fail on a type error, and
the helper's types could drift from pact-js indefinitely.

Now: `examples/js/product-consumer` has a `typecheck` script over all its `*.ts`, and the helper's
`npm test` runs `vitest run && npm run typecheck`.

`peerDependencies` moved from `>=12.0.0` to `>=17.1.4`, which is honest — the plugin interaction
chain and the error propagation this helper depends on do not exist in earlier majors.

### Status

Rust 125, helper 24 (+ typecheck), example 13 + 2 verification. Example typecheck exits 0, down
from 10 errors.


---

## Addendum 4 — deciding the pact-js type dependency (2026-09-07)

**Question:** can the initial release proceed without pact-js re-exporting its V4 plugin
interaction types, relying on a deep `./src` import instead?

**Answer: yes.** The upstream re-export is a nice-to-have, not a blocker.

### Evidence

| Check | Result |
|---|---|
| `exports` map in `@pact-foundation/pact` | **None** — in 15.0.1, 16.2.0 and 17.1.4 |
| `src/v4/http/types.d.ts` present | Yes, in all three majors |
| Deep import under `moduleResolution` `node` / `node16` / `nodenext` / `bundler` | **OK in all four** |
| `main` / `types` fields | `./src/index.js`, `./src/index.d.ts` |

Two facts do most of the work. First, `main` points *into* `src/`, so `src/` is pact-js's published
artifact rather than source that happens to ship — the path is far more stable than "reaching into
src" implies, and it has survived three majors. Second, with no `exports` map, subpath imports are
unrestricted under every resolver mode; `exports` is the only thing that would block them.

The decisive property, though, is the failure mode. These are **type-only** imports, erased at
compile time. If pact-js moves the path or adds an `exports` map, the result is a loud `TS2307` at
build time — never a broken consumer runtime.

### The chosen shape: mirror publicly, deep-import in a conformance check

Rather than either extreme, the helper does both:

- The **published** types stay structural mirrors, so no consumer ever resolves
  `@pact-foundation/pact/src/...`. Verified: the built `dist/*.d.ts` contains no import from
  pact-js at all.
- A **dev-only** `test/pact-js-conformance.ts` deep-imports the real types and asserts, at compile
  time, that each real type satisfies its mirror. It is type-only and emits nothing.

This keeps consumers decoupled while making drift impossible to miss: if pact-js changes the
chain, or adds an `exports` map that hides the path, `npm run typecheck` fails **in this repo
only**, and the fix is a local one.

The guard was verified to actually fire, in both directions:

- removing `pluginContents` from the request-builder mirror (the original bug) → errors at the two
  usage sites in `dsl.ts` and `index.ts`;
- adding a method to a mirror that pact-js does not have (an upstream rename) → errors *only* in
  the conformance file, which is precisely the drift no usage site could catch.

The second case is why the file earns its place: the three bugs in Addendum 3 were all of that
shape — a mirror that compiled fine against our own code while diverging from reality.

### Upstream ask, downgraded

Re-exporting the V4 plugin interaction types from the package root would let plugin authors import
them directly and delete the mirrors entirely. Still worth doing — it is the difference between
every plugin author hand-writing these types and none of them doing so — but it is no longer
blocking, and this repo now has a guard that makes the hand-written version safe in the meantime.

### Scope decision

Per maintainer direction, this plugin stays distinct from pact-js's built-in
`addGraphQLInteraction()` (the regex-matched, non-schema-aware V4 GraphQL interaction) for now. No
convergence work planned.
