import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, it, expect } from 'vitest';
import { PactV4 } from '@pact-foundation/pact';
import { buildSchema, parse, validate, isObjectType } from 'graphql';
import { graphqlHttpInteraction, graphqlInteraction, graphqlRequestBody } from 'pact-graphql-helper';

process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0';

const schema = readFileSync(resolve(__dirname, 'schema.graphql'), 'utf8');
const query = `
  query GetProduct($id: ID!) {
    product(id: $id) {
      id
      name
      status
    }
  }
`;

const graphqlSchema = buildSchema(schema);
// pact-js doesn't surface plugin validation errors, so validate queries locally.
const validateQuery = (source: string) => validate(graphqlSchema, parse(source));
const getUnknownFields = (typeName: string, value: Record<string, unknown>) => {
  const type = graphqlSchema.getType(typeName);
  if (!type || !isObjectType(type)) {
    throw new Error(`Expected ${typeName} to be an object type in schema`);
  }
  const fields = type.getFields();
  return Object.keys(value).filter((field) => !fields[field]);
};

const postGraphqlRequest = async (mockServer: { url: string }, request: unknown) =>
  fetch(`${mockServer.url}/graphql`, {
    method: 'POST',
    // Use application/graphql because the plugin matcher is keyed on this content type.
    headers: { 'content-type': 'application/graphql' },
    body: JSON.stringify(request),
  });

describe('GraphQL pact', () => {
  it('configures an interaction via the plugin', async () => {
    const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
    const interaction = pact.addInteraction();

    interaction.given('a product with ID 10 exists');
    interaction.uponReceiving('a GraphQL product request');

    const responseInteraction = await graphqlHttpInteraction(interaction, {
      schema,
      query,
      variables: { id: '10' },
      operationName: 'GetProduct',
    });

    await responseInteraction
      .willRespondWith(200, (builder) => {
        builder.headers({ 'content-type': 'application/json' });
        builder.jsonBody({
          data: {
            product: {
              id: '10',
              name: 'product name',
              status: 'ACTIVE',
            },
          },
        });
      })
      .executeTest(async (mockServer) => {
        await postGraphqlRequest(
          mockServer,
          graphqlRequestBody({
            query,
            variables: { id: '10' },
            operationName: 'GetProduct',
          }),
        );
      });
  });

  describe('positive cases', () => {
    it('supports nested product queries', async () => {
      const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
      const interaction = pact.addInteraction();

      interaction.given('a product catalog exists');
      interaction.uponReceiving('a nested product query');

      const nestedQuery = `
        query ProductsByStatus {
          products(status: ACTIVE) {
            id
            name
            status
            category {
              id
              name
            }
            variants {
              id
              sku
              price {
                list {
                  amount
                  currency
                }
              }
              inventory {
                quantity
                updatedAt
              }
            }
            reviews {
              id
              rating
              author {
                id
                name
              }
            }
          }
        }
      `;

      const responseInteraction = await graphqlHttpInteraction(interaction, {
        schema,
        query: nestedQuery,
        operationName: 'ProductsByStatus',
      });

      await responseInteraction
        .willRespondWith(200, (builder) => {
          builder.headers({ 'content-type': 'application/json' });
          builder.jsonBody({
            data: {
              products: [
                {
                  id: 'prod-1',
                  name: 'Trail Backpack',
                  status: 'ACTIVE',
                  category: {
                    id: 'cat-1',
                    name: 'Bags',
                  },
                  variants: [
                    {
                      id: 'var-1',
                      sku: 'SKU-TRAIL-001',
                      price: {
                        list: {
                          amount: 129.99,
                          currency: 'USD',
                        },
                      },
                      inventory: {
                        quantity: 42,
                        updatedAt: '2026-03-08T12:00:00Z',
                      },
                    },
                  ],
                  reviews: [
                    {
                      id: 'rev-1',
                      rating: 'FIVE',
                      author: {
                        id: 'cust-1',
                        name: 'Alex',
                      },
                    },
                  ],
                },
              ],
            },
          });
        })
        .executeTest(async (mockServer) => {
          await postGraphqlRequest(
            mockServer,
            graphqlRequestBody({
              query: nestedQuery,
              operationName: 'ProductsByStatus',
            }),
          );
        });
    });

    it('supports placeOrder mutation', async () => {
      const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
      const interaction = pact.addInteraction();

      interaction.given('a customer is ready to order');
      interaction.uponReceiving('a placeOrder mutation');

      const mutation = `
        mutation PlaceOrder {
          placeOrder(
            input: {
              customerId: "cust-1"
              shippingAddress: {
                line1: "123 Market St"
                city: "San Francisco"
                state: "CA"
                postalCode: "94103"
                country: "US"
              }
              lines: [
                { productId: "prod-1", variantId: "var-1", quantity: 2 }
                { productId: "prod-2", quantity: 1 }
              ]
              paymentToken: "tok_abc123"
            }
          ) {
            id
            status
            total {
              amount
              currency
            }
            lines {
              quantity
              product {
                id
                name
              }
              variant {
                id
                sku
              }
            }
            shipments {
              id
              status
              address {
                city
                country
              }
            }
          }
        }
      `;

      const responseInteraction = await graphqlHttpInteraction(interaction, {
        schema,
        query: mutation,
        operationName: 'PlaceOrder',
      });

      await responseInteraction
        .willRespondWith(200, (builder) => {
          builder.headers({ 'content-type': 'application/json' });
          builder.jsonBody({
            data: {
              placeOrder: {
                id: 'order-1',
                status: 'PLACED',
                total: {
                  amount: 289.97,
                  currency: 'USD',
                },
                lines: [
                  {
                    quantity: 2,
                    product: {
                      id: 'prod-1',
                      name: 'Trail Backpack',
                    },
                    variant: {
                      id: 'var-1',
                      sku: 'SKU-TRAIL-001',
                    },
                  },
                  {
                    quantity: 1,
                    product: {
                      id: 'prod-2',
                      name: 'Weekender Tote',
                    },
                    variant: {
                      id: 'var-2',
                      sku: 'SKU-WEEK-004',
                    },
                  },
                ],
                shipments: [
                  {
                    id: 'ship-1',
                    status: 'PENDING',
                    address: {
                      city: 'San Francisco',
                      country: 'US',
                    },
                  },
                ],
              },
            },
          });
        })
        .executeTest(async (mockServer) => {
          await postGraphqlRequest(
            mockServer,
            graphqlRequestBody({
              query: mutation,
              operationName: 'PlaceOrder',
            }),
          );
        });
    });

    it('accepts subscription document validation', async () => {
      const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
      const interaction = pact.addInteraction();

      interaction.given('inventory updates are available');
      interaction.uponReceiving('an inventoryChanged subscription document');

      const subscription = `
        subscription InventoryChanged {
          inventoryChanged(variantId: "var-1") {
            quantity
            updatedAt
          }
        }
      `;

      // Schema validation only; subscriptions aren't executed over HTTP in this example.
      const pluginInteraction = await graphqlInteraction(interaction, {
        schema,
        query: subscription,
        operationName: 'InventoryChanged',
      });
      expect(pluginInteraction).toBeDefined();
    });
  });

  describe('schema invalid', () => {
    it('rejects unknown field selections', async () => {
      const invalidQuery = `
        query InvalidField($id: ID!) {
          product(id: $id) {
            id
            name
            stockLevel
          }
        }
      `;

      const errors = validateQuery(invalidQuery);
      expect(errors).not.toHaveLength(0);
      expect(errors.some((error) => /Cannot query field "stockLevel"/.test(error.message))).toBe(
        true,
      );
    });

    it('rejects invalid enum values', async () => {
      const invalidEnumQuery = `
        query InvalidEnumValue {
          products(status: DISCONTINUED) {
            id
            name
            status
          }
        }
      `;

      const errors = validateQuery(invalidEnumQuery);
      expect(errors).not.toHaveLength(0);
      expect(
        errors.some(
          (error) =>
            error.message.includes('ProductStatus') && error.message.includes('DISCONTINUED'),
        ),
      ).toBe(true);
    });
  });

  describe('runtime mismatch', () => {
    it('rejects queries that omit nested selections from the canonical document', async () => {
      const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });
      const interaction = pact.addInteraction();

      interaction.given('a product catalog exists');
      interaction.uponReceiving('a product query missing nested selections');

      // Configure pact with the canonical query, then send a runtime query that omits fields.
      const canonicalQuery = `
        query ProductsByStatus {
          products(status: ACTIVE) {
            id
            name
            status
            category {
              id
              name
            }
          }
        }
      `;

      const runtimeQuery = `
        query ProductsByStatus {
          products(status: ACTIVE) {
            id
            name
            status
          }
        }
      `;

      const responseInteraction = await graphqlHttpInteraction(interaction, {
        schema,
        query: canonicalQuery,
        operationName: 'ProductsByStatus',
      });

      await expect(
        responseInteraction
          .willRespondWith(200, (builder) => {
            builder.headers({ 'content-type': 'application/json' });
            builder.jsonBody({
              data: {
                products: [
                  {
                    id: 'prod-1',
                    name: 'Trail Backpack',
                    status: 'ACTIVE',
                    category: {
                      id: 'cat-1',
                      name: 'Bags',
                    },
                  },
                ],
              },
            });
          })
          .executeTest(async (mockServer) => {
            await postGraphqlRequest(
              mockServer,
              graphqlRequestBody({
                query: runtimeQuery,
                operationName: 'ProductsByStatus',
              }),
            );
          }),
      ).rejects.toThrow(/is not selected by the actual query/);
    });
  });

  // TODO(plan-2): once the JS DSL forwards response_body_json to the plugin, this
  // assertion moves to the plugin and this local check can be deleted.
  describe('response not in schema', () => {
    it('rejects responses with unknown fields', async () => {
      const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider-negative' });
      const interaction = pact.addInteraction();

      interaction.given('a product with ID 10 exists');
      interaction.uponReceiving('a GraphQL product request with extra response fields');

      const responseInteraction = await graphqlHttpInteraction(interaction, {
        schema,
        query,
        variables: { id: '10' },
        operationName: 'GetProduct',
      });

      const responseBody = {
        data: {
          product: {
            id: '10',
            name: 'product name',
            status: 'ACTIVE',
            internalSku: 'INT-001',
          },
        },
      };

      // Pact JS doesn't surface response schema violations; validate locally.
      const unknownFields = getUnknownFields('Product', responseBody.data.product);
      expect(unknownFields).toEqual(['internalSku']);

      await responseInteraction
        .willRespondWith(200, (builder) => {
          builder.headers({ 'content-type': 'application/json' });
          builder.jsonBody(responseBody);
        })
        .executeTest(async (mockServer) => {
          await postGraphqlRequest(
            mockServer,
            graphqlRequestBody({
              query,
              variables: { id: '10' },
              operationName: 'GetProduct',
            }),
          );
        });
    });
  });
});
