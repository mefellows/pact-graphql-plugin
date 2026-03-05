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
    headers: { 'content-type': 'application/json' },
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

> Running the example requires the GraphQL plugin binary to be installed where Pact Core can find it (e.g. `$HOME/.pact/plugins`). Once available, the test will produce a pact showing a GraphQL request configured via the helper.

## Repo Status

See `docs/plans/2026-03-04-graphql-plugin-design.md` and the implementation plan in `docs/plans/2026-03-04-graphql-plugin-plan.md` for the current roadmap.
