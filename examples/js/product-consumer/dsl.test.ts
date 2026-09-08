import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, it, expect } from 'vitest';
import { PactV4 } from '@pact-foundation/pact';
import { graphql, gql } from '@pact-foundation/pact-graphql-plugin';

const schema = readFileSync(resolve(__dirname, 'schema.graphql'), 'utf8');

const newPact = () =>
  new PactV4({ consumer: 'product-consumer', provider: 'product-provider' });

describe('GraphQL DSL', () => {
  it('declares the query once and replays it through the bound client', async () => {
    const api = graphql(newPact(), { schema });

    await api
      .interaction('a GraphQL product request via the DSL')
      .given('a product with ID 10 exists')
      .query(gql`
        query GetProduct($id: ID!) {
          product(id: $id) {
            id
            name
            status
          }
        }
      `)
      .operationName('GetProduct')
      .variables({ id: '10' })
      .willRespondWith({
        data: { product: { id: '10', name: 'product name', status: 'ACTIVE' } },
      })
      .executeTest(async (client) => {
        // No query, no variables, no content-type, no path repeated here: the client replays
        // exactly what the interaction declared, so the two cannot drift.
        const result = await client.execute<{
          data: { product: { id: string; status: string } };
        }>();

        expect(result.data.product.id).toBe('10');
        expect(result.data.product.status).toBe('ACTIVE');
      });
  });

  it('serves GraphQL from a configurable path', async () => {
    // Its own provider, so this DSL-level concern does not put an interaction the example
    // provider server does not serve into the pact that `verify-provider.test.ts` verifies.
    const pact = new PactV4({
      consumer: 'product-consumer',
      provider: 'product-provider-custom-path',
    });
    const api = graphql(pact, { schema, path: '/api/graphql' });

    await api
      .interaction('a GraphQL request on a custom path')
      .given('a product with ID 10 exists')
      .query(gql`
        query GetProduct($id: ID!) {
          product(id: $id) {
            id
          }
        }
      `)
      .variables({ id: '10' })
      .willRespondWith({ data: { product: { id: '10' } } })
      .executeTest(async (client) => {
        expect(client.url).toMatch(/\/api\/graphql$/);
        const result = await client.execute<{ data: { product: { id: string } } }>();
        expect(result.data.product.id).toBe('10');
      });
  });

  // These assert on the *plugin's* rejection, which reaches the caller as a thrown error thanks
  // to pact-js-core >= 20.1.1 (pact-foundation/pact-js-core#956). Before that fix the FFI status
  // code was discarded and a rejected interaction was silently recorded with an empty part.
  it('rejects a query that selects a field absent from the schema', async () => {
    const api = graphql(newPact(), { schema });

    await expect(
      api
        .interaction('an invalid query')
        .query(gql`
          query InvalidField($id: ID!) {
            product(id: $id) {
              id
              stockLevel
            }
          }
        `)
        .variables({ id: '10' })
        .build(),
    ).rejects.toThrow(/stockLevel/);
  });

  it('rejects variables that do not satisfy the operation', async () => {
    const api = graphql(newPact(), { schema });

    await expect(
      api
        .interaction('a query with a mistyped variable name')
        .query(gql`
          query GetProduct($id: ID!) {
            product(id: $id) {
              id
            }
          }
        `)
        // `$id` is declared and required, but the author supplied `productId`.
        .variables({ productId: '10' })
        .build(),
    ).rejects.toThrow(/\$id/);
  });
});
