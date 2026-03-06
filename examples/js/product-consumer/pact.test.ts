import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, it } from 'vitest';
import { PactV4 } from '@pact-foundation/pact';
import { graphqlInteraction } from 'pact-graphql-helper';

const schema = readFileSync(resolve(__dirname, 'schema.graphql'), 'utf8');
const query = `
  query GetProduct($id: ID!) {
    product(id: $id) {
      id
      name
      type
    }
  }
`;

describe('GraphQL pact', () => {
  it('configures an interaction via the plugin', async () => {
    const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
    const interaction = pact.addInteraction();

    interaction.given('a product with ID 10 exists');
    interaction.uponReceiving('a GraphQL product request');

    const pluginInteraction = await graphqlInteraction(interaction, {
      schema,
      query,
      variables: { id: '10' },
      operationName: 'GetProduct',
    });
    const responseInteraction = pluginInteraction.withRequest('POST', '/graphql', (builder) => {
      builder.headers({ 'content-type': 'application/json' });
      builder.pluginContents(
        'application/json',
        JSON.stringify({
          query_document: query.trim(),
          operation_name: 'GetProduct',
          variables_json: JSON.stringify({ id: '10' }),
          transport: 'json_body',
          schema_sdl: schema,
        }),
      );
    });

    await responseInteraction
      .willRespondWith(200, (builder) => {
        builder.headers({ 'content-type': 'application/json' });
        builder.jsonBody({
          data: {
            product: {
              id: '10',
              name: 'product name',
              type: 'product series',
            },
          },
        });
      })
      .executeTest(async (mockServer) => {
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
  });
});
