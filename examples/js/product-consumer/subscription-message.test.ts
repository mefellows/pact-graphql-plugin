import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';
import { PactV4 } from '@pact-foundation/pact';
import { graphqlMessageInteraction } from 'pact-graphql-helper';

process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0';

const schema = readFileSync(resolve(__dirname, 'schema.graphql'), 'utf8');

describe('GraphQL message pact', () => {
  it('creates a subscription message pact envelope', async () => {
    const pactDir = resolve(__dirname, 'pacts', 'messages');
    const pactPath = resolve(pactDir, 'product-consumer-product-provider.json');
    const pact = new PactV4({
      consumer: 'product-consumer',
      provider: 'product-provider',
      dir: pactDir,
    });
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
      expect(message.contents.content).toEqual({
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
