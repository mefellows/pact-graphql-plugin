import type {
  GraphqlApi,
  GraphqlApiOptions,
  GraphqlClient,
  GraphqlInteractionBuilder,
  GraphqlInteractionWithPluginRequest,
  GraphqlInteractionWithPluginResponse,
  GraphqlPactBuilder,
  GraphqlQueryMatching,
  GraphqlRequestWithPluginBuilder,
  GraphqlResponseWithPluginBuilder,
  GraphqlTransport,
  GraphqlUnconfiguredInteraction,
  FetchLike,
} from './types';

/**
 * The plugin version this helper is built against. Overridable per-API for pinning, or via
 * `PACT_GRAPHQL_PLUGIN_VERSION` for CI, but a consumer author should never have to think about it.
 */
export const DEFAULT_PLUGIN_VERSION = '0.1.0';

const GRAPHQL_REQUEST_CONTENT_TYPE = 'application/graphql';
const GRAPHQL_RESPONSE_CONTENT_TYPE = 'application/graphql-response';

/**
 * Tagged template for GraphQL documents. It returns the source unchanged — its only job is to let
 * editors and language servers recognise the block as GraphQL. Parsing and canonicalisation are
 * the plugin's, deliberately: a second implementation here is a second source of truth.
 */
export function gql(strings: TemplateStringsArray, ...values: unknown[]): string {
  return strings.reduce(
    (out, chunk, index) => out + chunk + (index < values.length ? String(values[index]) : ''),
    '',
  );
}

interface InteractionState {
  description: string;
  given: string[];
  query?: string;
  operationName?: string;
  variables?: unknown;
  matching?: GraphqlQueryMatching;
  response?: { body: unknown; status: number };
}

function pluginConfiguration(
  api: ResolvedApiOptions,
  state: InteractionState,
): Record<string, unknown> {
  if (!state.query || state.query.trim().length === 0) {
    throw new Error('a GraphQL query is required — call .query(...) before building');
  }

  return {
    // Passed through verbatim. The plugin canonicalises both the expectation and the actual
    // request, so normalising here would only add a way for the two to disagree.
    query_document: state.query,
    operation_name: state.operationName,
    variables_json: state.variables === undefined ? undefined : JSON.stringify(state.variables),
    transport: api.transport,
    schema_sdl: api.schema,
    query_matching: state.matching,
  };
}

interface ResolvedApiOptions {
  schema?: string;
  path: string;
  transport: GraphqlTransport;
  pluginVersion: string;
  fetch: FetchLike;
}

class InteractionBuilder implements GraphqlInteractionBuilder {
  private readonly state: InteractionState;

  constructor(
    private readonly pact: GraphqlPactBuilder,
    private readonly api: ResolvedApiOptions,
    description: string,
  ) {
    this.state = { description, given: [] };
  }

  given(state: string): this {
    this.state.given.push(state);
    return this;
  }

  query(document: string): this {
    this.state.query = document;
    return this;
  }

  operationName(name: string): this {
    this.state.operationName = name;
    return this;
  }

  variables(variables: unknown): this {
    this.state.variables = variables;
    return this;
  }

  matching(mode: GraphqlQueryMatching): this {
    this.state.matching = mode;
    return this;
  }

  willRespondWith(body: unknown, status = 200): this {
    this.state.response = { body, status };
    return this;
  }

  async build(): Promise<
    GraphqlInteractionWithPluginRequest | GraphqlInteractionWithPluginResponse
  > {
    const configuration = pluginConfiguration(this.api, this.state);

    let interaction: GraphqlUnconfiguredInteraction = this.pact.addInteraction();
    for (const state of this.state.given) {
      interaction = interaction.given(state);
    }
    interaction = interaction.uponReceiving(this.state.description);

    // `usingPlugin` only loads the plugin — it is synchronous, and its `PluginConfig` carries no
    // configuration. The configuration reaches the plugin through `pluginContents` below, which is
    // also where validation fires.
    const withPlugin = interaction.usingPlugin({
      plugin: 'graphql',
      version: this.api.pluginVersion,
    });

    const withRequest = withPlugin.withRequest(
      'POST',
      this.api.path,
      (builder: GraphqlRequestWithPluginBuilder) => {
        builder.headers({ 'content-type': GRAPHQL_REQUEST_CONTENT_TYPE });
        builder.pluginContents(GRAPHQL_REQUEST_CONTENT_TYPE, JSON.stringify(configuration));
      },
    );

    if (!this.state.response) {
      return withRequest;
    }

    // The response-side configure call is a *separate* invocation sharing no state with the
    // request-side one, so it must carry the whole operation itself — otherwise selection-set
    // validation runs against the wrong document, and the plugin sees an operation whose declared
    // variables (`$id: ID!`) are unsatisfied and rejects the interaction.
    const responseConfiguration = {
      query_document: configuration.query_document,
      operation_name: configuration.operation_name,
      variables_json: configuration.variables_json,
      schema_sdl: this.api.schema,
      response_body_json: JSON.stringify(this.state.response.body),
    };

    return withRequest.willRespondWith(
      this.state.response.status,
      (builder: GraphqlResponseWithPluginBuilder) => {
        // Set BEFORE pluginContents: without an explicit content-type header, pact_ffi's HTTP
        // callback falls back to the content type passed to the FFI call itself
        // (application/graphql-response), which is not what a GraphQL server sends.
        builder.headers({ 'content-type': 'application/json' });
        builder.pluginContents(
          GRAPHQL_RESPONSE_CONTENT_TYPE,
          JSON.stringify(responseConfiguration),
        );
      },
    );
  }

  async executeTest<T>(handler: (client: GraphqlClient) => Promise<T>): Promise<T | undefined> {
    const interaction = (await this.build()) as GraphqlInteractionWithPluginResponse;

    return interaction.executeTest(async (mockServer: { url: string }) => {
      const client = this.createClient(mockServer.url);
      return handler(client);
    });
  }

  private createClient(baseUrl: string): GraphqlClient {
    const url = `${baseUrl.replace(/\/$/, '')}${this.api.path}`;
    const state = this.state;
    const doFetch = this.api.fetch;

    return {
      url,
      async execute<TData = any>(overrides?: {
        variables?: unknown;
        operationName?: string;
      }): Promise<TData> {
        // Replays what the interaction declared, so the request under test and the expectation
        // cannot drift. Overrides exist for negative tests that deliberately send something else.
        const body = {
          query: state.query,
          variables: overrides?.variables ?? state.variables,
          operationName: overrides?.operationName ?? state.operationName,
        };

        const response = await doFetch(url, {
          method: 'POST',
          headers: { 'content-type': GRAPHQL_REQUEST_CONTENT_TYPE },
          body: JSON.stringify(body),
        });

        return (await response.json()) as TData;
      },
    };
  }
}

class Api implements GraphqlApi {
  private readonly options: ResolvedApiOptions;

  constructor(
    private readonly pact: GraphqlPactBuilder,
    options: GraphqlApiOptions,
  ) {
    this.options = {
      schema: options.schema,
      path: options.path ?? '/graphql',
      transport: options.transport ?? 'json_body',
      pluginVersion:
        options.pluginVersion ??
        process.env.PACT_GRAPHQL_PLUGIN_VERSION ??
        DEFAULT_PLUGIN_VERSION,
      fetch: options.fetch ?? ((globalThis as any).fetch as FetchLike),
    };
  }

  interaction(description: string): GraphqlInteractionBuilder {
    return new InteractionBuilder(this.pact, this.options, description);
  }
}

/**
 * Entry point for the GraphQL DSL. The schema, path and plugin version are stated once for the
 * whole pact; each interaction then contributes only GraphQL and expectations.
 */
export function graphql(pact: GraphqlPactBuilder, options: GraphqlApiOptions = {}): GraphqlApi {
  return new Api(pact, options);
}
