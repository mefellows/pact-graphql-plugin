# Pact GraphQL Plugin

This repository hosts the WIP GraphQL Pact plugin plus supporting tooling.

## Components

- `pact-graphql-plugin/`: Rust plugin server implementing the Pact plugin protocol for GraphQL requests.
- `js/pact-graphql-helper/`: TypeScript helper that lets Pact consumers hand a GraphQL document + variables to the plugin without hand-crafting HTTP request bodies.

## Quick Start (JS Consumer)

The `graphql(...)` DSL states the schema and endpoint once per pact; each interaction then
contributes only GraphQL and expectations.

```ts
import { readFileSync } from 'node:fs';
import { PactV4 } from '@pact-foundation/pact';
import { graphql, gql } from 'pact-graphql-helper';

const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
const api = graphql(pact, { schema: readFileSync('schema.graphql', 'utf8') });

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
  .operationName('GetProduct')
  .variables({ id: '10' })
  .willRespondWith({
    data: { product: { id: '10', name: 'product name', status: 'ACTIVE' } },
  })
  .executeTest(async (client) => {
    // `client` is bound to the mock server, the configured path and content type, and replays
    // exactly what this interaction declared — so the request under test and the expectation
    // cannot drift.
    const { data } = await client.execute();
    expect(data.product.id).toBe('10');
  });
```

What the plugin does with that:

- **Validates the query** against the schema at pact-write time.
- **Validates the variables** against the operation's declared variable definitions — a missing
  `$id`, a `String` where an `Int` is declared, or a value outside an enum are all rejected.
- **Validates the expected response** against the schema and the query's selection set.
- **Derives matching rules** from the schema — `match: type` for scalars, a `match: regex` over
  the enum's values for enum fields — so the response is matched by type, not by literal equality.
- **Compares queries on the AST**, so formatting, comments and fragment structure do not cause
  spurious mismatches.

### Options

| Call | Purpose |
|------|---------|
| `graphql(pact, { schema, path, transport, pluginVersion })` | Per-pact setup. `path` defaults to `/graphql`. |
| `.given(state)` | Provider state; may be called more than once. |
| `.query(document)` / `.operationName(name)` / `.variables(vars)` | The operation. Passed to the plugin verbatim — canonicalisation is the plugin's job. |
| `.matching('exact' \| 'semantic' \| 'subset')` | Query comparison mode. Defaults to the core's `semantic`. `subset` lets the actual query request fewer fields than expected. |
| `.willRespondWith(body, status?)` | Expected GraphQL envelope. Status defaults to `200`. |
| `.executeTest(fn)` / `.build()` | Run the interaction, or configure it without running (useful for asserting rejections). |

### Rejections surface as test failures

Requires `@pact-foundation/pact` >= 17.1.4 (`pact-core` >= 20.1.1). Earlier versions discarded the
FFI status code, so a plugin rejection was silently recorded as an interaction with an empty part
and the test still passed (fixed by pact-foundation/pact-js-core#956).

```ts
await expect(
  api.interaction('an invalid query')
    .query(gql`query GetProduct($id: ID!) { product(id: $id) { id stockLevel } }`)
    .variables({ id: '10' })
    .build(),
).rejects.toThrow(/stockLevel/);
```

The thrown message carries the plugin's own diagnosis:

```
Failed to set plugin interaction contents for content type 'application/graphql':
the plugin returned an error: ... message: "GraphQL query validation failed:
field `stockLevel` does not exist on type `Product`"
```

Note that validation runs when the plugin *contents* are set, not when the plugin is loaded — so
`graphqlInteraction` alone (which only calls `usingPlugin`) does not trigger it. The DSL's
`.build()` / `.executeTest()` and `graphqlHttpInteraction` both do.

## Legacy helper API

`graphqlInteraction` / `graphqlHttpInteraction` remain available and are still exercised by
`examples/js/product-consumer/pact.test.ts`. New tests should prefer the DSL above.


```bash
npm install --save-dev @pact-foundation/pact @pact-foundation/pact-graphql-helper
```

```ts
import { PactV4 } from '@pact-foundation/pact';
import { graphqlInteraction } from '@pact-foundation/pact-graphql-helper';

const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
const interaction = pact.addInteraction('fetch a product via GraphQL');
const query = `
    query GetProduct($id: ID!) {
      product(id: $id) {
        id
        name
        type
      }
    }
  `;

interaction.given('a product with ID 10 exists');
interaction.uponReceiving('a GraphQL product request');

const pluginInteraction = await graphqlInteraction(interaction, {
  schema: readFileSync('schema.graphql', 'utf8'),
  query,
  variables: { id: '10' },
  operationName: 'GetProduct',
});

pluginInteraction.willRespondWith(200, (builder) => {
  builder.jsonBody({
    data: {
      product: {
        id: '10',
        name: 'product name',
        type: 'product series',
      },
    },
  });
});

await pact.executeTest(async (mockServer) => {
  await fetch(`${mockServer.url}/graphql`, {
    method: 'POST',
    headers: { 'content-type': 'application/graphql' },
    body: JSON.stringify({
      query,
      variables: { id: '10' },
      operationName: 'GetProduct',
    }),
  });
});
```

> The example above hand-builds the expected response with `jsonBody`, which Pact core matches
> with plain equality/type rules and the plugin never validates. Prefer the `response` option
> shown below so the response is checked against the schema/selection set too.

## Configuring the response through the plugin

`graphqlHttpInteraction` accepts an optional `response` (and `status`, default `200`) field:

```ts
const pluginInteraction = await graphqlHttpInteraction(interaction, {
  schema,
  query,
  variables: { id: '10' },
  operationName: 'GetProduct',
  response: {
    data: {
      product: { id: '10', name: 'product name', status: 'ACTIVE' },
    },
  },
});

await pluginInteraction.executeTest(async (mockServer) => { /* ... */ });
```

When `response` is supplied, the helper calls `willRespondWith`/`pluginContents` on the caller's
behalf so the response body is sent to the plugin for validation and the plugin derives Pact
matching rules from the schema (e.g. `match: type` for scalar fields, a `match: regex` enum-value
check for enums) instead of the response being matched by plain equality.

### Why the plugin is configured twice per HTTP interaction

A GraphQL HTTP interaction makes **two** separate `usingPlugin`/`configure_interaction` calls to
the plugin — one for the request part (`content_type: application/graphql`) and one for the
response part (`content_type: application/graphql-response`). This is not optional plumbing, it's
required by how `pact_ffi` applies plugin-configured content for `Synchronous_HTTP` interactions:
`pactffi_interaction_contents(interaction, part, content_type, contents)` takes an
`InteractionPart` enum (`Request` or `Response`) chosen by the *caller*, and the FFI applies
`contents.first()` from whatever the plugin returns to that part — it does not honour a
`part_name` field inside the plugin's response, and it does not accept two parts from one call.
An earlier version of this plugin tried to return both parts from a single `configure_interaction`
call (keyed by `part_name: "request"` / `part_name: "response"`); the FFI silently applied the
first part to whichever side the caller had asked for and discarded the second. **Each part of an
HTTP interaction must come from its own `configure_interaction` call.**

The response-side `configure_interaction` call also intentionally returns
`plugin_configuration: None`. Pact's V4 interaction model stores plugin configuration in a single
`plugin_config` field keyed only by plugin name, not by part — a second call that returned a
non-empty `plugin_configuration` would silently overwrite (not merge with) the first call's config,
which is what carries the request's `variables_json`. Returning `None` avoids that clobbering, and
is also semantically correct: the response part needs no stored plugin config, because its matching
is performed by Pact core's own JSON matcher against the matching rules the plugin attaches
directly to the part, not by calling back into the plugin.

One consequence of this design: the response body in the generated pact file is recorded with
`content-type: application/json` — the real content type a GraphQL server sends — not
`application/graphql-response`. The latter is only ever the content type passed to the
*configure* call, so that pact files stay valid for provider verification against a real server.

## Example Consumer

A complete example lives under `examples/js/product-consumer`. It links to the helper via a local `file:` dependency so you can exercise the workflow before the package is published.

```bash
cd examples/js/product-consumer
npm install
npm run test
```

> Running the example requires the GraphQL plugin binary to be discoverable by Pact Core. Run `just install` from the repo root to copy the current build into `~/.pact/plugins/graphql-<version>/` before executing the tests.

## Distribution

`pact-plugin.json` is the manifest the Pact plugin driver reads when bundling or loading the GraphQL plugin. The `version` stays at `0.0.0` in source control and is rewritten by the release workflow so packaged artifacts advertise the correct version alongside the compiled binary. Run `just version` to print the resolved plugin version as derived from Cargo metadata.

### Bundling prerequisites

- Rust toolchains plus the desired targets installed via `rustup target add <triple>`.
- CLI tools: `cargo`, `just`, `jq`, `gzip`, and `shasum` (or a compatible SHA-256 utility).
- POSIX-compatible `sh` on Windows hosts (Git Bash or WSL both work) so the recipes can execute.

### Bundling commands

- `just bundle <triple>` (the `target=<triple>` spelling also works) builds the release binary for the specified target, copies it into `dist/<triple>/pact-graphql-plugin[.exe]`, rewrites `pact-plugin.json` with the current version, and emits a gzip + `.sha256` pair named `pact-graphql-plugin-<os>-<arch>[.exe].gz` under the same directory.
- `just bundle-all` iterates over every entry in `SUPPORTED_TARGETS` and invokes the recipe above, producing a complete `dist/` tree in one go.

Each run is idempotent: rerunning a bundle overwrites the staged binary, manifest, archive, and checksum for that target.

### Local install

- `just install` detects the current host triple (via `rustc -Vv`), ensures a bundle for that triple exists (building one if necessary), extracts the gzip into a temporary directory, and copies both the binary and `pact-plugin.json` into `~/.pact/plugins/graphql-<version>/`.

The recipe requires the same CLI tools listed above (`jq`, `gzip`, `mktemp`, `rustup`) plus a POSIX-compatible shell on Windows (Git Bash or WSL). After running it, Pact CLI tooling will load the GraphQL plugin from the installed directory.

## Repo Status

See `docs/plans/2026-03-04-graphql-plugin-design.md` and the implementation plan in `docs/plans/2026-03-04-graphql-plugin-plan.md` for the current roadmap.
