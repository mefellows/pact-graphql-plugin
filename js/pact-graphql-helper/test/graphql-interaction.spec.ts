import { describe, expect, it } from 'vitest';

import { graphqlInteraction } from '../src/index';

function createFakeBuilder() {
  const calls: any[] = [];
  return {
    calls,
    usingPlugin: (options: any) => {
      calls.push(options);
      return Promise.resolve(options);
    },
  };
}

describe('graphqlInteraction', () => {
  it('builds plugin configuration for JSON transport', async () => {
    const builder = createFakeBuilder();

    await graphqlInteraction(builder, {
      schema: 'type Query { ping: String }',
      query: `
        query Ping($id: ID!) {
          ping(id: $id) { id }
        }
      `,
      operationName: 'PingQuery',
      variables: { id: '10' },
    });

    expect(builder.calls).toHaveLength(1);
    const call = builder.calls[0];
    expect(call.plugin).toBe('graphql');
    expect(call.version).toBe('0.0.0');
    expect(call.configuration.transport).toBe('json_body');
    expect(call.configuration.query_document).toContain('query Ping');
    expect(call.configuration.operation_name).toBe('PingQuery');
    expect(call.configuration.schema_sdl).toContain('type Query');
    expect(call.configuration.variables_json).toBe(JSON.stringify({ id: '10' }));
  });

  it('supports query string transport with pre-stringified variables', async () => {
    const builder = createFakeBuilder();
    const variables = JSON.stringify({ search: 'prod' }, null, 2);

    await graphqlInteraction(builder, {
      query: `
        query {
          products {
            id
          }
        }
      `,
      transport: 'query_string',
      variables,
    });

    const call = builder.calls[0];
    expect(call.configuration.transport).toBe('query_string');
    expect(call.configuration.variables_json).toBe(variables);
    expect(call.configuration.query_document).toContain('\n  products');
  });

  it('throws when query is missing', async () => {
    const builder = createFakeBuilder();
    await expect(
      graphqlInteraction(builder, {
        query: '   ',
      }),
    ).rejects.toThrow('GraphQL query is required');
  });

  it('throws when pre-stringified variables are invalid JSON', async () => {
    const builder = createFakeBuilder();

    await expect(
      graphqlInteraction(builder, {
        query: 'query { ping }',
        variables: '{"id": }',
      }),
    ).rejects.toThrow('variables string must contain valid JSON');
  });
});
