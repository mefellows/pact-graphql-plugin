import { describe, expect, it } from 'vitest';

import { graphqlMessageInteraction } from '../src/index';

function createFakeMessagePact() {
  const calls: any[] = [];
  return {
    calls,
    addAsynchronousInteraction: () => {
      const message: Record<string, any> = {
        contents: undefined,
        pluginContents: undefined,
      };

      const unconfigured = {
        expectsToReceive: (
          description: string,
          builder: (msgBuilder: { withJSONContent: (body: unknown) => void }) => void,
        ) => {
          message.description = description;
          builder({
            withJSONContent: (body: unknown) => {
              message.contents = { content: body };
            },
          });
          return { executeTest: async () => message };
        },
        usingPlugin: (_options: unknown) => ({
          withPluginContents: (contents: string, contentType: string) => {
            message.pluginContents = { contentType, contents };
            return { executeTest: async () => message };
          },
          expectsToReceive: (description: string) => {
            message.description = description;
            return { withPluginContents: (contents: string, contentType: string) => {
              message.pluginContents = { contentType, contents };
              return { executeTest: async () => message };
            } };
          },
        }),
      };

      calls.push(message);
      return unconfigured;
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

    expect(result.executeTest).toBeInstanceOf(Function);
    expect(pact.calls).toHaveLength(1);
    const call = pact.calls[0];

    expect(call.contents.content).toEqual({
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
