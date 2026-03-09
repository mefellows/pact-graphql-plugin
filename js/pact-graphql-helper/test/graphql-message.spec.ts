import { describe, expect, it } from 'vitest';

import { graphqlMessageInteraction } from '../src/index';

function createFakeMessagePact() {
  const calls: any[] = [];
  return {
    calls,
    addAsyncMessage: () => {
      const message: Record<string, any> = {
        contents: undefined,
        pluginContents: undefined,
      };
      const interaction = {
        pluginContents: (contentType: string, contents: string) => {
          message.pluginContents = { contentType, contents };
        },
        withContents: (contentTypeOrContents: unknown, contents?: unknown) => {
          if (contents === undefined) {
            message.contents = contentTypeOrContents;
          } else {
            message.contents = contents;
            message.contentType = contentTypeOrContents;
          }
          return message;
        },
      };
      calls.push(message);
      return interaction;
    },
  };
}

describe('graphqlMessageInteraction', () => {
  it('builds the message envelope and plugin contents', async () => {
    const pact = createFakeMessagePact();
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

    const result = await graphqlMessageInteraction(pact, {
      schema: 'type Subscription { inventoryChanged(variantId: ID!): Inventory }',
      subscription,
      operationName: 'InventoryChanged',
      variables: { variantId: 'var-1' },
      data,
    });

    expect(result).toBe(pact.calls[0]);
    expect(pact.calls).toHaveLength(1);
    const call = pact.calls[0];

    expect(call.contentType).toBe('application/json');
    expect(call.contents).toEqual({
      subscription: 'InventoryChanged',
      variables: { variantId: 'var-1' },
      data,
    });

    expect(call.pluginContents.contentType).toBe('application/graphql');
    const pluginPayload = JSON.parse(call.pluginContents.contents);
    expect(pluginPayload).toEqual({
      query_document:
        'subscription InventoryChanged($variantId: ID!) {\n  inventoryChanged(variantId: $variantId) {\n    quantity\n    updatedAt\n  }\n}',
      operation_name: 'InventoryChanged',
      variables_json: JSON.stringify({ variantId: 'var-1' }),
      transport: 'json_body',
      schema_sdl: 'type Subscription { inventoryChanged(variantId: ID!): Inventory }',
    });
  });
});
