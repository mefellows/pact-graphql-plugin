import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { resolve } from 'node:path';
import { MessageProviderPact, Verifier } from '@pact-foundation/pact';

import { buildInventoryChangedEvent, startProviderServer } from './provider-server';

process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0';

describe('GraphQL provider verification', () => {
  let server: Awaited<ReturnType<typeof startProviderServer>>;

  beforeAll(async () => {
    server = await startProviderServer();
  });

  afterAll(async () => {
    await server.close();
  });

  it('builds inventory message payload from provider logic', () => {
    const event = buildInventoryChangedEvent('var-1');
    expect(event).toEqual({
      subscription: 'InventoryChanged',
      variables: { variantId: 'var-1' },
      data: {
        inventoryChanged: { quantity: 42, updatedAt: '2026-03-08T12:00:00Z' },
      },
    });
  });

  it('verifies HTTP and message pacts', async () => {
    const httpPactPath = resolve(
      __dirname,
      'pacts',
      'product-consumer-product-provider.json',
    );
    const messagePactPath = resolve(
      __dirname,
      'pacts',
      'messages',
      'product-consumer-product-provider.json',
    );

    const httpVerifier = new Verifier({
      provider: 'product-provider',
      providerBaseUrl: server.url,
      pactUrls: [httpPactPath],
    });

    const messagePact = new MessageProviderPact({
      provider: 'product-provider',
      pactUrls: [messagePactPath],
      messageProviders: {
        'a GraphQL subscription event': async () =>
          buildInventoryChangedEvent('var-1'),
      },
    });

    await httpVerifier.verifyProvider();
    await messagePact.verify();
  });
});
