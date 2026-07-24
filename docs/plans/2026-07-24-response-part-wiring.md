# Wiring GraphQL Response Matching Through Pact Core

> **For agentic workers:** implement task-by-task. Steps use checkbox (`- [ ]`) syntax. Task 1 is a
> spike whose findings may invalidate Tasks 2-5 as written — **do not skip it, and stop to report if
> its findings contradict the assumptions recorded below.**

**Goal:** Make the response-side validation built in Tasks 6-8 of
`2026-07-22-graphql-core-semantics.md` actually reachable from a consumer test, so that a response
body containing a field absent from the schema (or absent from the query's selection set) fails the
test *through the plugin* rather than through a local `validate()` call in the example.

**Background — why the current wiring cannot work.** Task 8 made `configure_interaction` return two
`InteractionResponse` parts from a single call, `part_name: "request"` and `part_name: "response"`
(`pact-graphql-plugin/src/server.rs:75-137`). That pattern is only honoured for
`Synchronous_Messages`. For `Synchronous_HTTP`, `pact_ffi` does:

```rust
// pact-reference/rust/pact_ffi/src/plugins/mod.rs:216-237
let part = get_part(interaction, part);      // the part the CALLER named
if let Some(contents) = contents.first() {   // only the FIRST returned part
```

It ignores `part_name` entirely and applies `contents.first()` to whichever part the caller passed
via the `InteractionPart` argument. Our second part is silently discarded. `Asynchronous_Messages`
(line 238) does the same. Only `setup_sync_message_contents` (line 292) filters on `part_name`.

The existing tests in `pact-graphql-plugin/tests/plugin_flow.rs` pass because they drive our gRPC
service directly and never go through `pact_ffi` — that is the blind spot that let this through.

**The seam that already exists.** `pactffi_interaction_contents(interaction, part, content_type,
contents)` takes an `InteractionPart`. The intended model for HTTP is **one configure call per
part**. pact-js already exposes both sides — `RequestWithPluginBuilder.pluginContents` →
`withPluginRequestInteractionContents`, and `ResponseWithPluginBuilder.pluginContents` →
`withPluginResponseInteractionContents`. The reference `matt` plugin uses exactly this
(`node_modules/@pact-foundation/pact/src/pact.integration.spec.js:140-160`). **No new FFI, and no
change to any client language's native layer, is required.**

## Assumptions to be verified by Task 1

These drive the design. Each is a reading of the source, not an observed behaviour. If Task 1
disproves one, stop and report rather than working around it.

- **A1** — A response-side `pluginContents('application/graphql-response', ...)` call reaches our
  plugin's `configure_interaction` with `content_type = application/graphql-response`, because our
  `init_plugin` catalogue registers a `ContentMatcher` for that content type (`server.rs:186-193`)
  and `setup_contents` routes via `find_content_matcher(content_type)`.
- **A2** — Returning a single `InteractionResponse` from that call causes its body **and its
  matching rules** to be applied to the response part of the interaction.
- **A3** — An `Err` returned from `configure_interaction` surfaces to the JS caller as a thrown
  error. `pactffi_interaction_contents` returns code 6 and calls `set_error_msg` on failure
  (`plugins/mod.rs:281-285`), which looks more promising than the path that swallowed errors in
  review finding §1.3 — but this is unconfirmed.
- **A4** — `setup_contents`' HTTP callback adds a `content-type` header equal to **the content type
  the caller passed to the FFI**, not the body's own content type (`plugins/mod.rs:220-222`), and
  only when the part has no such header already. So without intervention the recorded response would
  carry `content-type: application/graphql-response`, which is wrong on the wire and would break
  provider verification against a real server. Setting the header explicitly in the DSL *before*
  `pluginContents` should prevent it.

## Design decision: matchers, not a response content matcher

Two designs were considered.

**A — matchers only (chosen).** The response part is recorded with `content-type: application/json`
and carries the matching rules derived by `derive_matching_rules`. Pact core's own JSON matcher
enforces them at verification time. Schema/selection-set violations in the *expected* body are
caught at `configure` time, where we return an error and fail the consumer test immediately.

**B — plugin response matcher (rejected).** Record the response with
`content-type: application/graphql-response` so core routes comparison back to our
`compare_contents`. Rejected because the pact file would then advertise a content type no real
GraphQL server sends, breaking provider verification.

Consequence of choosing A: the response branch of `compare_contents`
(`server.rs:238-280`) becomes unreachable for HTTP. **Do not delete it** — it is still the right code
for a future synchronous-message (GraphQL-over-WebSocket) path, and Task 5 covers documenting that.
The `graphql-response` `ContentMatcher` catalogue entry must stay regardless: it is what routes the
response-side *configure* call to us.

## Global Constraints

- Crate is `pact_graphql_plugin`; `cargo` runs from the repo root. Rust edition 2021.
- No new Rust dependencies. New items `pub(crate)` unless they appear in an already-`pub` type.
- Never `unwrap()`/`expect()` on user-derived input — return `anyhow::Result` or push a mismatch.
- Testing convention from the previous plan still holds: crate-internal tests live in sibling
  `*_tests.rs` files wired in with `#[cfg(test)] #[path = "..."] mod tests;`. There is no
  `test_support` module. `tests/plugin_flow.rs` is a genuine integration test.
- `cargo test` green and `cargo build` warning-free before every commit.
- Conventional Commits.
- **A passing `cargo test` is not sufficient evidence for this plan.** Every task that changes
  consumer-facing behaviour must be proven by running the JS example suite end-to-end.

## Running the example end-to-end

`just install` is currently broken: the `bundle` recipe's nested `just target-label {{target}}` call
passes the literal string `target=aarch64-apple-darwin` instead of the resolved value. Reproduce with
`just bundle target=aarch64-apple-darwin`. Until it is fixed, install by hand:

```bash
cargo build --release --target aarch64-apple-darwin
# copy the binary + pact-plugin.json into ~/.pact/plugins/graphql-0.1.0/
cd examples/js/product-consumer && npm install && npm run test
```

Fixing the Justfile is out of scope here but worth a follow-up.

---

### Task 1: Spike — prove the response-part round trip

No production code. The deliverable is knowledge plus one integration test.

**Files:**
- Create: `examples/js/product-consumer/spike.test.ts` (deleted again at the end of this task)

- [ ] **Step 1: Write the smallest possible end-to-end probe**

In `examples/js/product-consumer/`, write a test that bypasses the helper entirely and calls pact-js
directly, mirroring the `matt` plugin pattern:

```ts
await pact
  .addInteraction()
  .given('a product with ID 10 exists')
  .uponReceiving('a spike request')
  .usingPlugin({ plugin: 'graphql', version: '0.1.0' })
  .withRequest('POST', '/graphql', (b) => {
    b.headers({ 'content-type': 'application/graphql' });
    b.pluginContents('application/graphql', JSON.stringify(requestConfig));
  })
  .willRespondWith(200, (b) => {
    b.headers({ 'content-type': 'application/json' });   // set BEFORE pluginContents — see A4
    b.pluginContents('application/graphql-response', JSON.stringify(responseConfig));
  })
  .executeTest(async (mockServer) => { /* post the query, assert on the body */ });
```

`requestConfig` is what `buildGraphqlConfiguration` already produces. For `responseConfig`, pass the
same `query_document` / `operation_name` / `schema_sdl` plus `response_body_json` — the plugin needs
the query and schema to validate the selection set, and the response-side configure call is a
*separate* invocation that shares no state with the request-side one.

- [ ] **Step 2: Record what actually happens**

Run it with `PACT_LOG_LEVEL=debug` (or `LOG_LEVEL=debug`) and answer each question by observation,
not inference. Write the answers into this plan file under a new "## Task 1 findings" section:

1. Does our plugin's `configure` get invoked a second time with
   `content_type = application/graphql-response`? (A1)
2. Does the generated pact file's response body match what we returned, and does it carry the
   matching rules from `derive_matching_rules`? Inspect `pacts/*.json`. (A2)
3. What `content-type` header does the response part end up with — with the explicit `headers()`
   call, and without it? (A4)
4. When the response body contains a field not in the schema so `configure` returns an error, what
   does the JS caller see — a thrown error with our message, a silent pass, or something else? (A3)

- [ ] **Step 3: Report before continuing**

If A1 or A2 is false, the whole approach is wrong — **stop and report**, do not attempt Tasks 2-5.
If only A3 is false (errors do not surface), continue: Tasks 2-4 still deliver the matchers, and the
error-surfacing problem becomes a separate upstream issue to raise against pact-js. Note it clearly.

- [ ] **Step 4: Delete the spike file and commit the findings**

```bash
git add docs/plans/2026-07-24-response-part-wiring.md
git commit -m "docs: record response-part round-trip spike findings"
```

---

## Task 1 findings

**Method.** `examples/js/product-consumer/spike.test.ts` (now deleted) called `graphqlHttpInteraction`
for the request side (unchanged, known-good helper) and then called `willRespondWith(200, b =>
b.pluginContents('application/graphql-response', ...))` directly against the raw pact-js v4 builder,
mirroring the `matt` plugin pattern, across three variants: (a) explicit
`content-type: application/json` header before `pluginContents`, (b) no explicit header, (c) a
response body containing a field (`internalSku`) not in the schema. The request-side `configure` call
did **not** set `response_body_json`, per the task's instructions.

Getting evidence out of the *plugin process itself* required more than `LOG_LEVEL=debug`/`PACT_LOG_LEVEL=debug`
on the `vitest` invocation or `logLevel` on the `PactV4` constructor — both only raise the log level of
the **host** process's in-process `pact_ffi`/`pact_plugin_driver` (confirmed: with `logLevel: 'trace'`
on `PactV4`, `~/.pact/plugins/graphql-0.1.0/graphql-plugin.log` still recorded `env RUST_LOG=OFF` /
`env LOG_LEVEL=OFF` for the spawned plugin subprocess even though `LOG_LEVEL=DEBUG` was set in the
shell that launched `vitest`, and even though the host-side trace logging clearly *was* elevated —
`pact_ffi`/`pact_plugin_driver` do not forward the host's log level into the child plugin process's
environment). Worked around this for the plugin's own `debug!` output by temporarily replacing
`~/.pact/plugins/graphql-0.1.0/pact-graphql-plugin` with a wrapper shell script that force-set
`LOG_LEVEL=trace`/`RUST_LOG=trace` before `exec`-ing the real binary, then restored the original binary
afterwards. This is host-machine setup only — no repo files were affected by it. This is worth raising
as friction for the next person; not fixed here as it's out of scope for a "no production code" spike.

With that in place, the host-process trace log (`pact_ffi`'s own tracing, elevated via
`PactV4({ logLevel: 'trace' })`) and the plugin's own `graphql-plugin.log` together gave direct
observational evidence for all four questions.

### 1. Does a second `configure` call happen with `content_type = application/graphql-response`? (A1)

**CONFIRMED.** The plugin's own debug log shows two separate `configure_interaction` invocations per
interaction, the second with the response content type:

```
DEBUG configure_interaction: request received from Pact core
DEBUG configure: building GraphQL interaction content_type=application/graphql
...
DEBUG configure_interaction: request received from Pact core
DEBUG configure: building GraphQL interaction content_type=application/graphql-response
```

This pattern repeated identically for all three spike variants (six `configure_interaction` calls
total across three interactions, each pair `application/graphql` then `application/graphql-response`).
A1 holds.

### 2. Does the pact file's response body match what was returned, with the derived matching rules? (A2)

**FALSE under the current (pre-fix) plugin code — and not directly testable in the "single part"
form the assumption describes, because that form doesn't exist yet without a production-code change.**

What was observed: the host-side trace of the interaction state *after* the response-side `configure`
call, for the "valid response" variant, shows the response body is **not** the JSON envelope
(`{"data":{"product":{...}}}`) that was passed as `response_body_json`. It is the *request*-shaped
canonical GraphQL body — i.e. the first of the two `InteractionResponse` parts our (unfixed) `configure`
still returns for this call:

```
response: HttpResponse { status: 200, headers: Some({"content-type": ["application/json"]}),
  body: Present(b"{\"query\":\"query GetProduct($id: ID!) {\n  product(id: $id) {\n    id\n    name\n    status\n  }\n}\",\"operationName\":\"GetProduct\"}", ...),
  matching_rules: MatchingRules { rules: {HEADER: ..., STATUS: ...} }, ... }
```

Note there is no `BODY` category in `matching_rules` at all — the `derive_matching_rules` output,
which our code attaches only to the *second* list entry (`part_name: "response"`), never reaches the
interaction, because `pactffi_interaction_contents` takes `contents.first()` regardless of `part_name`,
exactly as the Background section's source excerpt predicts. This is a direct, empirical confirmation
of the bug the plan diagnoses.

Because Task 1 is spike-only ("No production code"), the plugin could not be modified to actually
return a single part per call, so the literal claim in A2 — that a *single* returned `InteractionResponse`
gets applied correctly — could not be directly observed. Given get_part(interaction, part) selects the
part by the caller's `InteractionPart` argument and then unconditionally applies `contents.first()`,
returning exactly one part must by construction land on the part the caller asked for; Task 2 should
still confirm this from the JS side rather than take it on faith, since Task 1 has now shown the
current two-part code takes the *wrong* one.

**A second, more serious problem was discovered that A2 as framed does not cover:** the pact V4 model
stores plugin configuration as a single field per interaction, keyed only by plugin name
(`plugin_config: {"graphql": {...}}`), not per part. Each `configure_interaction` call's returned
`plugin_configuration` **overwrites** this shared field wholesale. Since the response-side `configure`
call's config (built from `query_document`/`schema_sdl`/`response_body_json`, with no
`variables_json`) is the *second* call, it clobbers the *first* (request-side) call's config, which had
the real `variables_json`. The stored config used later for matching/generating the request therefore
loses the variables:

```
"plugin_config": {"graphql": { ... "variables_json": null,
  "request": {"operation_name": "GetProduct", "query_document": "...", "transport": "json_body",
              "variables_json": null}, ... }}
```

and the mock server then reports a **request**-side mismatch when the actual request (which does carry
`variables`) is compared against this corrupted stored config:

```
1.0   /payload/variables_json: GraphQL variables_json differs
```

This happened in every spike variant that reached matching. **This is independent of the A1/A2
part-routing bug and will not be fixed by Task 2 as currently scoped** — Task 2 only changes which
single part each `configure` call returns; it does not change that pact core keeps only one shared
`plugin_config` blob per interaction and that the second `configure_interaction` call overwrites it.
Unless the response-side config is made to carry forward (or the plugin's returned
`plugin_configuration` is merged rather than replaced — which is core/FFI behaviour, not something the
plugin controls), a two-`configure`-call model will keep corrupting request-side matching state. This
needs to be resolved (e.g. by having the response-side `configure` call return a `plugin_configuration`
that preserves the fields the request side needs, or by confirming pact core actually merges per-part
data in a newer FFI version) **before** Tasks 2-4 proceed, or Task 2's "Verify" step needs an explicit
check for this regression.

### 3. What `content-type` header does the response part get, with/without the explicit `headers()` call? (A4)

**CONFIRMED as predicted.**

With `b.headers({ 'content-type': 'application/json' })` called before `pluginContents`:

```
response: HttpResponse { status: 200, headers: Some({"content-type": ["application/json"]}), ... }
```

Without it:

```
response: HttpResponse { status: 200, headers: Some({"content-type": ["application/graphql-response"]}), ... }
```

I.e. omitting the explicit header lets `setup_contents`' HTTP callback set `content-type` to the
content type the *caller* passed to the FFI call (`application/graphql-response`) — which is wrong on
the wire, exactly as A4 predicted. Setting the header explicitly beforehand does prevent this. A4 holds.

### 4. What does the JS caller see when `configure` returns an `Err`? (A3)

**FALSE — errors are silently swallowed, not thrown.** The plugin's own log confirms the response-side
`configure` call did fail:

```
ERROR pact_plugin_driver::content: Call to plugin failed - status: InvalidArgument, message:
  "GraphQL response validation failed: $.data.product.internalSku: `internalSku` is not a field of
  type \"Product\"", ...
ERROR pact_ffi::plugins: Failed to call out to plugin - Call to plugin failed - status: InvalidArgument,
  message: "GraphQL response validation failed: ...", ...
```

But the spike's `try { ... } catch (err) { thrown = err }` around the whole
`addInteraction()...pluginContents()...executeTest()` chain never caught anything —
`console.log('SPIKE thrown error:', thrown)` printed `undefined`, and `expect(thrown).toBeDefined()`
failed. `executeTest` ran to completion without throwing. The pact file that *was* written for this
interaction shows the response with no body at all — the failed `pluginContents` call silently left the
response part configured with only what `willRespondWith(200, ...)` set directly (status + nothing
else, since no `.headers()`/`.jsonBody()` fallback was called in this branch):

```json
"response": {
  "headers": { "content-type": ["application/json"] },
  "status": 200
}
```

So the observed behaviour is neither "thrown error with our message" nor a fully silent pass in the
sense of "test still exercises the intended assertion" — it's a **third outcome**: the error is logged
by the native layer and swallowed by the JS binding, the response body is silently dropped, and the
test proceeds and can still pass (with an empty/near-empty response part), giving no signal to the
author that anything went wrong. This matches the plan's contingency in Task 1 Step 3 ("if only A3 is
false... the error-surfacing problem becomes a separate upstream issue to raise against pact-js") but
is worse than a plain silent pass: the resulting pact file is subtly corrupt (a response with no body/
matching rules) rather than merely missing an assertion.

### Summary

| # | Assumption | Result |
|---|---|---|
| A1 | Response-side `pluginContents` reaches `configure_interaction` with `content_type = application/graphql-response` | **Holds** |
| A2 | A single returned `InteractionResponse` gets its body + matching rules applied to the response part | **Not directly testable without a production-code change; current two-part behaviour empirically confirms the diagnosed bug (wrong part applied, no BODY matching rules); a new, separate bug was found — the interaction's single shared `plugin_config` blob is clobbered by the second `configure_interaction` call, corrupting request-side matching data** |
| A3 | An `Err` from `configure_interaction` surfaces to the JS caller as a thrown error | **Does not hold** — the error is logged natively but swallowed at the JS binding; the test proceeds with a silently corrupted (bodyless) response part instead of throwing |
| A4 | Without an explicit header, the response's `content-type` becomes the content type passed to the FFI call | **Holds** |

---

### Task 2: One part per configure call, and stop clobbering the shared plugin config

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs` (the `configure` fn, currently lines 60-144)
- Modify: `pact-graphql-plugin/tests/plugin_flow.rs`

**The `plugin_config` clobbering fix (added after the Task 1 spike).** A V4 interaction stores plugin
configuration in a single `HashMap<String, HashMap<String, Value>>` keyed by *plugin name*, not by
part (`pact_models/src/v4/interaction.rs:109`). `pact_ffi` applies it with:

```rust
// pact-reference/rust/pact_ffi/src/plugins/mod.rs:229-231
if !contents.plugin_config.is_empty() {
  interaction.plugin_config_mut().insert(plugin_name, contents.plugin_config.interaction_configuration.clone());
}
```

`insert` overwrites. So in a two-call model the response-side call's config replaces the request-side
call's, and the request loses `variables_json` — the spike observed exactly this, producing a spurious
`/payload/variables_json: GraphQL variables_json differs` mismatch.

**The insert is guarded by `!contents.plugin_config.is_empty()`.** So the fix is entirely within our
control and requires no upstream change: **the response-side `configure` must return
`plugin_configuration: None`.** This is also semantically correct — under design decision A the
response part needs no stored config, because response matching is performed by core's JSON matcher
using the matching rules we attach to the part directly, and `generate_content` is never called for a
consumer-side response.

- [ ] **Step 1: Update the integration tests first**

The Task 8 tests in `plugin_flow.rs` assert that one `configure` call returns two parts. That
expectation is now known to be wrong. Rewrite them so that:

- a `configure` call with `content_type = application/graphql` returns exactly **one**
  `InteractionResponse`, `part_name: "request"`, regardless of whether `response_body_json` is set,
  and **does** carry `plugin_configuration`;
- a `configure` call with `content_type = application/graphql-response` and a valid
  `response_body_json` returns exactly **one** part, `part_name: "response"`, whose body is the
  response JSON with `content_type: "application/json"`, whose `rules` are the derived matchers, and
  whose `plugin_configuration` is **`None`** (this is the clobbering fix — assert it explicitly, it
  is load-bearing and non-obvious);
- a `configure` call with `content_type = application/graphql-response` and a response body
  violating the schema returns an `Err` naming the offending field path;
- a `configure` call with `content_type = application/graphql-response` and **no**
  `response_body_json` returns an `Err` saying it is required.

Run `cargo test` and confirm these fail.

- [ ] **Step 2: Branch `configure` on the content type**

Split the body of `configure` into a request path and a response path keyed on `req.content_type`
(compare with `starts_with(GRAPHQL_RESPONSE_CONTENT_TYPE)`, consistent with `compare_contents`).

The request path returns the single `"request"` part it already builds, with its
`plugin_configuration` as today. Delete the `if let Some(response_json) = ...` block that appends the
second part.

The response path requires `response_body_json`, resolves the schema, runs
`crate::response::validate_response`, returns an error listing every mismatch as `path: description`
if any, and otherwise returns the single `"response"` part with `derive_matching_rules` output —
i.e. the code currently at `server.rs:82-137`, moved and made the sole return value — with
`plugin_configuration: None` on both the `InteractionResponse` and the enclosing
`ConfigureInteractionResponse`.

Keep the response body's `content_type` as `application/json` (design decision A above), *not* the
GraphQL response content type.

- [ ] **Step 3: Verify**

`cargo test` green, `cargo build` warning-free.

Then verify end-to-end, because `cargo test` cannot see either bug this task fixes. Install the
plugin by hand (see "Running the example end-to-end") and run a consumer test that configures both
parts. Confirm from the generated pact file that:

- the response part's body is the GraphQL response envelope, not the request-shaped body;
- the response part has a `BODY` category in its `matchingRules`;
- `plugin_config.graphql.variables_json` is still populated (the clobbering regression);
- no spurious `/payload/variables_json` mismatch is reported.

If the helper does not yet support this (Task 3), do it with a temporary inline test as the spike did,
and delete it afterwards.

- [ ] **Step 4: Commit**

```bash
git commit -m "fix: return a single interaction part per configure call"
```

---

### Task 3: Teach the JS helper to configure the response part

**Files:**
- Modify: `js/pact-graphql-helper/src/types.ts`
- Modify: `js/pact-graphql-helper/src/index.ts`
- Modify: `js/pact-graphql-helper/test/graphql-interaction.spec.ts`

- [ ] **Step 1: Extend the option and builder types**

In `types.ts`, add to `GraphqlRequestOptions`:

```ts
  /** Expected GraphQL response envelope, e.g. `{ data: { product: {...} } }`. */
  response?: unknown;
  /** HTTP status for the expected response. Defaults to 200. */
  status?: number;
```

Add a `GraphqlHttpResponseBuilder` interface (`headers`, `pluginContents`, same shape as
`GraphqlHttpRequestBuilder`) and extend `GraphqlHttpInteractionBuilder` so the value returned by
`withRequest` exposes
`willRespondWith(status: number, builder: (b: GraphqlHttpResponseBuilder) => void): T`.

- [ ] **Step 2: Configure the response part when `response` is supplied**

In `index.ts`, extend `graphqlHttpInteraction`. After `withRequest`, when `options.response !==
undefined`, chain `willRespondWith`, setting the `content-type: application/json` header **before**
calling `pluginContents` (assumption A4), and passing a configuration object containing
`query_document`, `operation_name`, `schema_sdl` and `response_body_json:
JSON.stringify(options.response)`.

Factor the shared fields out of `buildGraphqlConfiguration` rather than duplicating the
normalisation — the response config must carry the *same* canonical query the request config did, or
selection-set validation will compare against the wrong document.

When `options.response` is undefined, behaviour is unchanged, so every existing caller keeps working.

- [ ] **Step 3: Unit-test the helper**

Add cases to `graphql-interaction.spec.ts` against the existing fake builder: `willRespondWith` is
not called when `response` is absent; when present it is called with the status (default 200), the
JSON content-type header is set before `pluginContents`, and the plugin contents carry
`response_body_json` plus the canonicalised query.

- [ ] **Step 4: Verify and commit**

`cd js/pact-graphql-helper && npm test`.

```bash
git commit -m "feat(js): configure the GraphQL response part through the plugin"
```

---

### Task 4: Make the examples prove it

This is the task that closes review finding §1.3. **This task is the point of the whole plan** — if
it cannot be completed, the previous three tasks have not delivered user-visible value.

**Files:**
- Modify: `examples/js/product-consumer/pact.test.ts`

- [ ] **Step 1: Route the happy-path interactions through the new option**

Convert the existing product-query tests to pass `response: { data: { product: {...} } }` instead of
hand-building the expected response with `jsonBody`. Confirm the generated pact still contains the
matching rules, now schema-derived.

- [ ] **Step 2: Rewrite the "response not in schema" test to assert on the plugin**

**BLOCKED — the Task 1 spike disproved A3. Do not attempt this step; leave the test as-is and keep
the TODO.** Retained here as documentation of what unblocks it.

The intent was: delete the local `getUnknownFields` helper and the `// Pact JS doesn't surface
response schema violations; validate locally.` comment, along with the `TODO(plan-2)` marker Task 8
added, and assert instead that building the interaction **rejects**, naming `internalSku`.

The spike found the plugin *does* return the right error and the native layer *does* log it, but the
JS caller never sees it. Root cause, confirmed by reading the shipped binding:

```js
// node_modules/@pact-foundation/pact-core/src/consumer/index.js:80-83
withPluginResponseInteractionContents: (contentType, contents) => {
    ffi.pactffiPluginInteractionContents(interactionPtr, INTERACTION_PART_RESPONSE, contentType, contents);
    return true;   // ← FFI return code discarded, success hardcoded
},
```

`pactffi_interaction_contents` returns `0` on success and `6` on plugin error, having already called
`set_error_msg` (`pact_ffi/src/plugins/mod.rs:281-285`). `pact-core` throws that code away and
hardcodes `true`; `ResponseWithPluginBuilder.pluginContents` then ignores its return value too. So the
error is discarded twice over, and the interaction is recorded with a bodyless response part — a
silently corrupt pact rather than a failed test.

Unblocking this needs an upstream fix in `@pact-foundation/pact-core`: check the return code and, when
non-zero, read the message via `pactffiGetErrorMessage` and throw. That is a small, contributable
change, but it is upstream of this repo and must not be faked locally.

- [ ] **Step 3: Add a test for a field the query did not select**

A response containing `name` when the query selected only `id` must reject. This case is impossible
to catch with raw Pact and is the clearest demonstration of the plugin's value.

- [ ] **Step 4: Run the full example suite**

`cd examples/js/product-consumer && npm run test` — including provider verification, which must
still pass against the recorded pact.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(examples): assert response schema violations through the plugin"
```

---

### Task 5: Documentation and follow-ups

- [ ] **Step 1: Document the two-call model**

In `README.md`, record that GraphQL HTTP interactions configure the plugin twice — once per part —
and why (core applies only the first returned part for HTTP). This is the single least obvious thing
about the design and the next person will otherwise repeat the Task 8 mistake.

- [ ] **Step 2: Comment the unreachable branch**

Add a comment above the response branch of `compare_contents` in `server.rs` explaining that it is
not reached by the HTTP flow (responses are matched by core's JSON matcher using our derived rules)
and is retained for a future synchronous-message transport.

- [ ] **Step 3: Update the objectives review**

In `docs/reviews/2026-07-22-objectives-review.md`, update §1.3 and §4.1 to reflect what now works.
Do not mark objectives met beyond what the example suite actually demonstrates.

- [ ] **Step 4: Commit**

```bash
git commit -m "docs: explain the per-part plugin configuration model"
```

---

## Out of scope

Deliberately excluded, still open from the objectives review:

- **Go binding parity.** `go/pactgraphql` needs the same response-part support. Worth doing right
  after this plan, and a good test of whether the design really is language-neutral.
- Stripping `normalizeQuery` / `serializeVariables` / `graphqlRequestBody` from the bindings (§3).
- Matchers inside `variables` (§4.3).
- The fluent builder and bound client from the review's "Updated examples" (§1.1).
- Subscription transport framing (§1.4).
- The `just install` / `just bundle` argument-passing bug.
