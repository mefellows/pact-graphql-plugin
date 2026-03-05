import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { pactWith } from '@pact-foundation/pact/v3';
import { graphqlInteraction } from 'pact-graphql-helper';

pactWith({ consumer: 'product-consumer', provider: 'product-provider' }, (interaction) => {
  interaction('fetch product via GraphQL', async (builder) => {
    await graphqlInteraction(builder, {
      schema: readFileSync(resolve(__dirname, 'schema.graphql'), 'utf8'),
      query: `
        query GetProduct($id: ID!) {
          product(id: $id) {
            id
            name
            type
          }
        }
      `,
      variables: { id: '10' },
      operationName: 'GetProduct',
    });

    builder.given('a product with ID 10 exists');
    builder.uponReceiving('a GraphQL product request');
    builder.willRespondWith({
      status: 200,
      headers: { 'content-type': 'application/json' },
      body: {
        data: {
          product: {
            id: '10',
            name: 'product name',
            type: 'product series',
          },
        },
      },
    });
  });
});
