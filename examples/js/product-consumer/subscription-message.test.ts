import { mkdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';
import { makeConsumerMessagePact } from '@pact-foundation/pact-core';
import { graphqlMessageInteraction } from 'pact-graphql-helper';

process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0';

const schema = readFileSync(resolve(__dirname, 'schema.graphql'), 'utf8');

const createMessagePact = (pactDir: string) => {
  const pact = makeConsumerMessagePact('product-consumer', 'product-provider', 4);

  return {
    addAsyncMessage: () => {
      const message = pact.newAsynchronousMessage('');
      message.expectsToReceive('a product inventory subscription event');

      return {
        pluginContents: (contentType: string, contents: string) => {
          const version = process.env.PACT_GRAPHQL_PLUGIN_VERSION ?? '0.0.0';
          pact.addPlugin('graphql', version);
          message.withPluginRequestInteractionContents(contentType, contents);
        },
        withContents: (contentTypeOrContents: unknown, contents?: unknown) => {
          const [contentType, body] =
            contents === undefined
              ? ['application/json', contentTypeOrContents]
              : [contentTypeOrContents as string, contents];

          message.withContents(JSON.stringify(body), contentType);

          return {
            executeTest: async (handler: (message: { contents: unknown }) => Promise<unknown>) => {
              const result = await handler({ contents: body });
              mkdirSync(pactDir, { recursive: true });
              pact.writePactFile(pactDir, true);
              return result;
            },
          };
        },
      };
    },
  };
};

describe('GraphQL message pact', () => {
  it('creates a subscription message pact envelope', async () => {
    const pactDir = resolve(__dirname, 'pacts', 'messages');
    const pactPath = resolve(pactDir, 'product-consumer-product-provider.json');
    const pact = createMessagePact(pactDir);
    const subscription = `
      subscription InventoryChanged($variantId: ID!) {
        inventoryChanged(variantId: $variantId) {
          quantity
          updatedAt
        }
      }
    `;
    const data = {
      inventoryChanged: {
        quantity: 42,
        updatedAt: '2026-03-08T12:00:00Z',
      },
    };

    const interaction = await graphqlMessageInteraction(pact, {
      schema,
      subscription,
      operationName: 'InventoryChanged',
      variables: { variantId: 'var-1' },
      data,
    });

    await interaction.executeTest(async (message) => {
      expect(message.contents).toEqual({
        subscription: 'InventoryChanged',
        variables: { variantId: 'var-1' },
        data,
      });
    });

    const pactJson = JSON.parse(readFileSync(pactPath, 'utf8')) as {
      metadata?: Record<string, unknown>;
    };
    expect(pactJson.metadata).toBeDefined();
  });
});
