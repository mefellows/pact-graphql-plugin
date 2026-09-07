import { describe, expect, it } from 'vitest';

import { graphql, gql } from '../src/index';

const SCHEMA = `
  schema { query: Query }
  type Product { id: ID! name: String! }
  type Query { product(id: ID!): Product }
`;

const QUERY = gql`
  query GetProduct($id: ID!) {
    product(id: $id) {
      id
      name
    }
  }
`;

/**
 * A stand-in for pact-js's V4 builder chain, recording what the DSL asks of it. The DSL's whole
 * job is to drive this chain correctly, so asserting on the recording is asserting on behaviour.
 */
function createFakePact() {
  const record: any = {
    given: [],
    description: undefined,
    plugin: undefined,
    request: undefined,
    response: undefined,
    executed: false,
  };

  const responseBuilder = {
    headers: (headers: Record<string, string>) => {
      record.response.headers = headers;
      record.response.order.push('headers');
    },
    pluginContents: (contentType: string, contents: string) => {
      record.response.pluginContents = { contentType, contents: JSON.parse(contents) };
      record.response.order.push('pluginContents');
    },
  };

  const withRequestResult: any = {
    willRespondWith: (status: number, handler: (b: any) => void) => {
      record.response = { status, order: [] };
      handler(responseBuilder);
      return withRequestResult;
    },
    executeTest: async (handler: (server: any) => Promise<unknown>) => {
      record.executed = true;
      return handler({ url: 'http://127.0.0.1:1234' });
    },
  };

  const pluginInteraction = {
    withRequest: (method: string, path: string, handler: (b: any) => void) => {
      record.request = { method, path };
      handler({
        headers: (headers: Record<string, string>) => {
          record.request.headers = headers;
        },
        pluginContents: (contentType: string, contents: string) => {
          record.request.pluginContents = { contentType, contents: JSON.parse(contents) };
        },
      });
      return withRequestResult;
    },
  };

  const interaction: any = {
    given: (state: string) => {
      record.given.push(state);
      return interaction;
    },
    uponReceiving: (description: string) => {
      record.description = description;
      return interaction;
    },
    // Synchronous, like pact-js's own `usingPlugin`, which returns `V4InteractionWithPlugin`.
    usingPlugin: (options: any) => {
      record.plugin = options;
      return pluginInteraction;
    },
  };

  return { record, pact: { addInteraction: () => interaction } };
}

describe('gql', () => {
  it('returns the document source unchanged', () => {
    const id = 'ID!';
    expect(gql`query Q($id: ${id}) { product(id: $id) { id } }`).toBe(
      'query Q($id: ID!) { product(id: $id) { id } }',
    );
  });
});

describe('graphql DSL', () => {
  it('sends the schema, query and variables to the plugin in one configure call', async () => {
    const { record, pact } = createFakePact();
    const api = graphql(pact as any, { schema: SCHEMA });

    await api
      .interaction('a GraphQL product request')
      .given('a product with ID 10 exists')
      .query(QUERY)
      .variables({ id: '10' })
      .build();

    expect(record.description).toBe('a GraphQL product request');
    expect(record.given).toEqual(['a product with ID 10 exists']);
    expect(record.plugin.plugin).toBe('graphql');
    expect(record.request.pluginContents.contents).toMatchObject({
      query_document: QUERY,
      variables_json: '{"id":"10"}',
      schema_sdl: SCHEMA,
    });
  });

  it('passes the query through verbatim rather than normalising it client-side', async () => {
    // Canonicalisation belongs to the plugin, which does it to both sides of the comparison.
    // Doing it here too would be a second, divergent implementation.
    const { record, pact } = createFakePact();
    const api = graphql(pact as any, { schema: SCHEMA });
    const indented = '\n      query Q { product(id: "1") { id } }\n   ';

    await api.interaction('x').query(indented).build();

    expect(record.request.pluginContents.contents.query_document).toBe(indented);
  });

  it('defaults the path to /graphql and allows an override', async () => {
    const { record, pact } = createFakePact();
    await graphql(pact as any, { schema: SCHEMA }).interaction('x').query(QUERY).build();
    expect(record.request.path).toBe('/graphql');
    expect(record.request.method).toBe('POST');

    const other = createFakePact();
    await graphql(other.pact as any, { schema: SCHEMA, path: '/api/graphql' })
      .interaction('x')
      .query(QUERY)
      .build();
    expect(other.record.request.path).toBe('/api/graphql');
  });

  it('exposes the query matching mode the core implements', async () => {
    const { record, pact } = createFakePact();

    await graphql(pact as any, { schema: SCHEMA })
      .interaction('x')
      .query(QUERY)
      .matching('subset')
      .build();

    expect(record.request.pluginContents.contents.query_matching).toBe('subset');
  });

  it('defaults the query matching mode to the core default rather than inventing one', async () => {
    const { record, pact } = createFakePact();
    await graphql(pact as any, { schema: SCHEMA }).interaction('x').query(QUERY).build();
    expect(record.request.pluginContents.contents.query_matching).toBeUndefined();
  });

  it('routes the expected response through the plugin for schema validation', async () => {
    const { record, pact } = createFakePact();

    await graphql(pact as any, { schema: SCHEMA })
      .interaction('x')
      .query(QUERY)
      .variables({ id: '10' })
      .willRespondWith({ data: { product: { id: '10', name: 'a name' } } })
      .build();

    expect(record.response.status).toBe(200);
    expect(record.response.pluginContents.contentType).toBe('application/graphql-response');
    expect(record.response.pluginContents.contents).toMatchObject({
      query_document: QUERY,
      schema_sdl: SCHEMA,
      response_body_json: '{"data":{"product":{"id":"10","name":"a name"}}}',
    });
  });

  it('sets the response content-type header before the plugin contents', async () => {
    // pact_ffi otherwise falls back to the configure call's content type, which is wrong on the wire.
    const { record, pact } = createFakePact();

    await graphql(pact as any, { schema: SCHEMA })
      .interaction('x')
      .query(QUERY)
      .willRespondWith({ data: { product: null } })
      .build();

    expect(record.response.order).toEqual(['headers', 'pluginContents']);
    expect(record.response.headers).toEqual({ 'content-type': 'application/json' });
  });

  it('accepts a non-200 status', async () => {
    const { record, pact } = createFakePact();
    await graphql(pact as any, { schema: SCHEMA })
      .interaction('x')
      .query(QUERY)
      .willRespondWith({ errors: [{ message: 'nope' }] }, 400)
      .build();
    expect(record.response.status).toBe(400);
  });

  it('hands executeTest a client bound to the mock server', async () => {
    const { pact } = createFakePact();
    const sent: any[] = [];

    await graphql(pact as any, {
      schema: SCHEMA,
      // Injected so the test asserts on what would go over the wire without a real server.
      fetch: (async (url: string, init: any) => {
        sent.push({ url, init });
        return {
          ok: true,
          status: 200,
          json: async () => ({ data: { product: { id: '10', name: 'a name' } } }),
        };
      }) as any,
    })
      .interaction('x')
      .query(QUERY)
      .variables({ id: '10' })
      .willRespondWith({ data: { product: { id: '10', name: 'a name' } } })
      .executeTest(async (client) => {
        const result = await client.execute();
        expect(result.data.product.id).toBe('10');
      });

    expect(sent).toHaveLength(1);
    expect(sent[0].url).toBe('http://127.0.0.1:1234/graphql');
    expect(sent[0].init.method).toBe('POST');
    expect(sent[0].init.headers['content-type']).toBe('application/graphql');
    // The client replays exactly what was declared once, so consumer and expectation cannot drift.
    expect(JSON.parse(sent[0].init.body)).toEqual({
      query: QUERY,
      variables: { id: '10' },
      operationName: undefined,
    });
  });

  it('sends the declared operationName when one is given', async () => {
    const { pact } = createFakePact();
    const sent: any[] = [];

    await graphql(pact as any, {
      schema: SCHEMA,
      fetch: (async (url: string, init: any) => {
        sent.push({ url, init });
        return { ok: true, status: 200, json: async () => ({ data: {} }) };
      }) as any,
    })
      .interaction('x')
      .query(QUERY)
      .operationName('GetProduct')
      .variables({ id: '10' })
      .executeTest(async (client) => {
        await client.execute();
      });

    expect(JSON.parse(sent[0].init.body).operationName).toBe('GetProduct');
  });

  it('requires a query', async () => {
    const { pact } = createFakePact();
    await expect(graphql(pact as any, { schema: SCHEMA }).interaction('x').build()).rejects.toThrow(
      /query/i,
    );
  });

  it('supports multiple given states', async () => {
    const { record, pact } = createFakePact();
    await graphql(pact as any, { schema: SCHEMA })
      .interaction('x')
      .given('state one')
      .given('state two')
      .query(QUERY)
      .build();
    expect(record.given).toEqual(['state one', 'state two']);
  });

  it('serialises variables without a client-side JSON implementation of its own', async () => {
    const { record, pact } = createFakePact();
    await graphql(pact as any, { schema: SCHEMA })
      .interaction('x')
      .query(QUERY)
      .variables({ b: 2, a: 1 })
      .build();
    // Key order is preserved here; the plugin canonicalises it on both sides.
    expect(record.request.pluginContents.contents.variables_json).toBe('{"b":2,"a":1}');
  });
});

describe('response-side configure call', () => {
  it('carries the variables so the operation validates as a whole', async () => {
    // The response-side call is a separate `configure_interaction` invocation sharing no state
    // with the request-side one. Omitting the variables makes the plugin see an operation whose
    // declared `$id: ID!` is unsatisfied, and it rejects the whole interaction.
    const { record, pact } = createFakePact();

    await graphql(pact as any, { schema: SCHEMA })
      .interaction('x')
      .query(QUERY)
      .variables({ id: '10' })
      .willRespondWith({ data: { product: { id: '10', name: 'a name' } } })
      .build();

    expect(record.response.pluginContents.contents.variables_json).toBe('{"id":"10"}');
  });
});
