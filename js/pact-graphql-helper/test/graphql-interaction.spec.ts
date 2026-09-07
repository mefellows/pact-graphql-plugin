import { describe, expect, it } from 'vitest';

import { graphqlHttpInteraction, graphqlInteraction, graphqlRequestBody } from '../src/index';

function createFakeBuilder() {
  const calls: any[] = [];
  return {
    calls,
    usingPlugin: (options: any) => {
      calls.push(options);
      return options;
    },
  };
}

function makeRequestPart(method: string, path: string, handler: (builder: any) => void) {
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
  return request;
}

function attachWillRespondWith(request: Record<string, any>, calls: any[]) {
  request.willRespondWith = (status: number, handler: (builder: any) => void) => {
    const response: Record<string, any> = {
      status,
      headers: undefined,
      pluginContents: undefined,
      order: [],
    };
    const builder = {
      headers: (headers: Record<string, string>) => {
        response.headers = headers;
        response.order.push('headers');
      },
      pluginContents: (contentType: string, contents: string) => {
        response.pluginContents = { contentType, contents };
        response.order.push('pluginContents');
      },
    };
    handler(builder);
    request.response = response;
    calls.push(response);
    return request;
  };
  return request;
}

function createFakeHttpInteraction() {
  const calls: any[] = [];
  return {
    calls,
    // pact-js's `usingPlugin` is synchronous: it returns `V4InteractionWithPlugin`, not a promise.
    usingPlugin: (options: any) => {
      calls.push({ plugin: options });
      return {
        withRequest: (method: string, path: string, handler: (builder: any) => void) => {
          const request = makeRequestPart(method, path, handler);
          calls.push(request);
          return attachWillRespondWith(request, calls);
        },
      };
    },
    withRequest: (method: string, path: string, handler: (builder: any) => void) => {
      const request = makeRequestPart(method, path, handler);
      calls.push(request);
      return attachWillRespondWith(request, calls);
    },
  };
}

describe('graphqlInteraction', () => {
  // `usingPlugin` only loads the plugin: pact-js's `PluginConfig` is `{plugin, version}` and its
  // implementation calls `addPlugin(plugin, version)`. Earlier tests here asserted on a
  // `configuration` property passed to `usingPlugin`, which pact-js silently discarded — so they
  // proved nothing about what the plugin actually received.
  it('passes only the plugin name and version to usingPlugin', () => {
    const builder = createFakeBuilder();

    graphqlInteraction(builder, {
      schema: 'type Query { ping: String }',
      query: 'query Ping($id: ID!) { ping(id: $id) { id } }',
      operationName: 'Ping',
      variables: { id: '10' },
    });

    expect(builder.calls).toHaveLength(1);
    expect(builder.calls[0]).toEqual({ plugin: 'graphql', version: '0.1.0' });
  });

  it('validates the options eagerly even though nothing is sent yet', () => {
    const builder = createFakeBuilder();

    expect(() => graphqlInteraction(builder, { query: '   ' })).toThrow(/query is required/i);
  });

  it('throws when pre-stringified variables are invalid JSON', () => {
    const builder = createFakeBuilder();

    expect(() =>
      graphqlInteraction(builder, {
        query: 'query Ping { ping }',
        variables: '{ not json',
      }),
    ).toThrow(/valid JSON/);
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

  it('does not configure a response part when `response` is absent', async () => {
    const interaction = createFakeHttpInteraction();

    const result = await graphqlHttpInteraction(interaction, {
      schema: 'type Query { ping: String }',
      query: 'query { ping }',
    });

    expect(result.response).toBeUndefined();
    expect(interaction.calls.map((call: any) => call.pluginContents?.contentType)).not.toContain(
      'application/graphql-response',
    );
  });

  it('configures the response part via the plugin when `response` is supplied', async () => {
    const interaction = createFakeHttpInteraction();

    const result = await graphqlHttpInteraction(interaction, {
      schema: 'type Query { product(id: ID!): Product } type Product { id: ID! name: String }',
      query: `
        query GetProduct($id: ID!) {
          product(id: $id) { id name }
        }
      `,
      operationName: 'GetProduct',
      variables: { id: '10' },
      response: { data: { product: { id: '10', name: 'Backpack' } } },
    });

    expect(result.response).toBeDefined();
    expect(result.response.status).toBe(200);
    // The content-type header must be set BEFORE pluginContents is called (assumption A4).
    expect(result.response.order).toEqual(['headers', 'pluginContents']);
    expect(result.response.headers).toEqual({ 'content-type': 'application/json' });
    expect(result.response.pluginContents.contentType).toBe('application/graphql-response');

    const responsePayload = JSON.parse(result.response.pluginContents.contents);
    expect(responsePayload).toEqual({
      query_document: 'query GetProduct($id: ID!) {\n  product(id: $id) { id name }\n}',
      operation_name: 'GetProduct',
      // Carried so the plugin sees `$id: ID!` as satisfied on this separate configure call.
      variables_json: '{"id":"10"}',
      schema_sdl: 'type Query { product(id: ID!): Product } type Product { id: ID! name: String }',
      response_body_json: JSON.stringify({ data: { product: { id: '10', name: 'Backpack' } } }),
    });
  });

  it('defaults the response status to 200 and honours an explicit status', async () => {
    const interaction = createFakeHttpInteraction();

    const result = await graphqlHttpInteraction(interaction, {
      schema: 'type Query { ping: String }',
      query: 'query { ping }',
      response: { data: { ping: 'pong' } },
      status: 404,
    });

    expect(result.response.status).toBe(404);
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
