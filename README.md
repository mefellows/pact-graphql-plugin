# Pact GraphQL Plugin

This repository hosts the WIP GraphQL Pact plugin plus supporting tooling.

## Components

- `pact-graphql-plugin/`: Rust plugin server implementing the Pact plugin protocol for GraphQL requests.
- `js/pact-graphql-helper/`: TypeScript helper that lets Pact consumers hand a GraphQL document + variables to the plugin without hand-crafting HTTP request bodies.

## Quick Start (JS Consumer)

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

- `just bundle target=<triple>` builds the release binary for the specified target, copies it into `dist/<triple>/pact-graphql-plugin[.exe]`, rewrites `pact-plugin.json` with the current version, and emits a gzip + `.sha256` pair named `pact-graphql-plugin-<os>-<arch>[.exe].gz` under the same directory.
- `just bundle-all` iterates over every entry in `SUPPORTED_TARGETS` and invokes the recipe above, producing a complete `dist/` tree in one go.

Each run is idempotent: rerunning a bundle overwrites the staged binary, manifest, archive, and checksum for that target.

### Local install

- `just install` detects the current host triple (via `rustc -Vv`), ensures a bundle for that triple exists (building one if necessary), extracts the gzip into a temporary directory, and copies both the binary and `pact-plugin.json` into `~/.pact/plugins/graphql-<version>/`.

The recipe requires the same CLI tools listed above (`jq`, `gzip`, `mktemp`, `rustup`) plus a POSIX-compatible shell on Windows (Git Bash or WSL). After running it, Pact CLI tooling will load the GraphQL plugin from the installed directory.

## Repo Status

See `docs/plans/2026-03-04-graphql-plugin-design.md` and the implementation plan in `docs/plans/2026-03-04-graphql-plugin-plan.md` for the current roadmap.
