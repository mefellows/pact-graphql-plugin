import { describe, expect, it } from 'vitest';

import { graphqlHttpInteraction, graphqlInteraction, graphqlRequestBody } from '../src/index';

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

function createFakeHttpInteraction() {
  const calls: any[] = [];
  return {
    calls,
    usingPlugin: (options: any) => {
      calls.push({ plugin: options });
      return Promise.resolve({
        withRequest: (method: string, path: string, handler: (builder: any) => void) => {
          const request: Record<string, any> = {
            method,
            path,
            headers: undefined,
            pluginContents: undefined,
          };
          const builder = {
            headers: (headers: Record<string, string>) => {
              request.headers = headers;
            },
            pluginContents: (contentType: string, contents: string) => {
              request.pluginContents = { contentType, contents };
            },
          };
          handler(builder);
          calls.push(request);
          return request;
        },
      });
    },
    withRequest: (method: string, path: string, handler: (builder: any) => void) => {
      const request: Record<string, any> = {
        method,
        path,
        headers: undefined,
        pluginContents: undefined,
      };
      const builder = {
        headers: (headers: Record<string, string>) => {
          request.headers = headers;
        },
        pluginContents: (contentType: string, contents: string) => {
          request.pluginContents = { contentType, contents };
        },
      };
      handler(builder);
      calls.push(request);
      return request;
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

describe('graphqlHttpInteraction', () => {
  it('builds HTTP request with plugin contents', async () => {
    const interaction = createFakeHttpInteraction();

    const result = await graphqlHttpInteraction(interaction, {
      schema: 'type Query { ping(id: ID!): Ping } type Ping { id: ID! }',
      query: `
        query Ping($id: ID!) {
          ping(id: $id) { id }
        }
      `,
      operationName: 'PingQuery',
      variables: { id: '10' },
    });

    expect(result).toBe(interaction.calls[1]);
    expect(interaction.calls).toHaveLength(2);
    const call = interaction.calls[1];
    expect(call.method).toBe('POST');
    expect(call.path).toBe('/graphql');
    expect(call.headers).toEqual({ 'content-type': 'application/graphql' });
    expect(call.pluginContents.contentType).toBe('application/graphql');

    const payload = JSON.parse(call.pluginContents.contents);
    expect(payload).toEqual({
      query_document: 'query Ping($id: ID!) {\n  ping(id: $id) { id }\n}',
      operation_name: 'PingQuery',
      variables_json: JSON.stringify({ id: '10' }),
      transport: 'json_body',
      schema_sdl: 'type Query { ping(id: ID!): Ping } type Ping { id: ID! }',
    });
  });
});

describe('graphqlRequestBody', () => {
  it('normalizes queries and parses variables', () => {
    const body = graphqlRequestBody({
      query: `
        query Ping($id: ID!) {
          ping(id: $id) { id }
        }
      `,
      operationName: 'PingQuery',
      variables: JSON.stringify({ id: '10' }),
    });

    expect(body).toEqual({
      query: 'query Ping($id: ID!) {\n  ping(id: $id) { id }\n}',
      variables: { id: '10' },
      operationName: 'PingQuery',
    });
  });
});
