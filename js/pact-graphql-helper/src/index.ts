import type {
  GraphqlAsyncMessageBuilder,
  GraphqlAsyncMessageWithPluginContents,
  GraphqlInteractionWithPlugin,
  GraphqlInteractionWithPluginRequest,
  GraphqlInteractionWithPluginResponse,
  GraphqlMessageOptions,
  GraphqlMessagePactBuilder,
  GraphqlRequestOptions,
  GraphqlRequestWithPluginBuilder,
  GraphqlResponseWithPluginBuilder,
  GraphqlTransport,
  GraphqlUnconfiguredInteraction,
} from './types';

const GRAPHQL_RESPONSE_CONTENT_TYPE = 'application/graphql-response';

function normalizeQuery(query: string): string {
  const lines = query.replace(/\r?\n/g, '\n').split('\n');
  let minIndent = Number.POSITIVE_INFINITY;

  for (const line of lines) {
    if (!line.trim()) {
      continue;
    }
    const match = line.match(/^(\s*)\S/);
    if (match) {
      minIndent = Math.min(minIndent, match[1].length);
    } else {
      minIndent = 0;
    }
  }

  if (!Number.isFinite(minIndent)) {
    minIndent = 0;
  }

  const dedented = lines.map((line) => {
    if (!line.trim()) {
      return '';
    }
    return line.slice(minIndent);
  });

  return dedented.join('\n').trim();
}

function serializeVariables(variables: unknown): string | undefined {
  if (variables === undefined) {
    return undefined;
  }

  if (typeof variables === 'string') {
    try {
      JSON.parse(variables);
      return variables;
    } catch {
      throw new Error('variables string must contain valid JSON');
    }
  }

  try {
    return JSON.stringify(variables);
  } catch {
    throw new Error('variables value must be JSON serialisable');
  }
}

function buildEnvelopeVariables(variables: unknown): unknown {
  if (variables === undefined) {
    return undefined;
  }

  if (typeof variables === 'string') {
    try {
      return JSON.parse(variables);
    } catch {
      throw new Error('variables string must contain valid JSON');
    }
  }

  return variables;
}

export function graphqlRequestBody(options: GraphqlRequestOptions) {
  const variables = buildEnvelopeVariables(options.variables);
  return {
    query: normalizeQuery(options.query),
    variables,
    operationName: options.operationName,
  };
}

function resolveTransport(transport?: GraphqlTransport): GraphqlTransport {
  return transport ?? 'json_body';
}

function buildGraphqlConfiguration(options: GraphqlRequestOptions) {
  if (!options.query || options.query.trim().length === 0) {
    throw new Error('GraphQL query is required');
  }

  return {
    query_document: normalizeQuery(options.query),
    operation_name: options.operationName,
    variables_json: serializeVariables(options.variables),
    transport: resolveTransport(options.transport),
    schema_sdl: options.schema,
  };
}

/**
 * Loads the GraphQL plugin for an interaction.
 *
 * Note this does **not** send any configuration to the plugin: pact-js's `usingPlugin` only calls
 * `addPlugin(plugin, version)` and its `PluginConfig` has no `configuration` field, so the plugin
 * is not consulted until the interaction *contents* are set. Schema and query validation therefore
 * do not run here.
 *
 * @deprecated Prefer `graphql()` (see `./dsl`), or `graphqlHttpInteraction` for the older API.
 */
export function graphqlInteraction(
  builder: GraphqlUnconfiguredInteraction,
  options: GraphqlRequestOptions,
): GraphqlInteractionWithPlugin {
  // Validates the options eagerly so a malformed call still fails here rather than silently.
  buildGraphqlConfiguration(options);

  return builder.usingPlugin({
    plugin: 'graphql',
    version: pluginVersion(),
  });
}

function pluginVersion(): string {
  return process.env.PACT_GRAPHQL_PLUGIN_VERSION ?? '0.1.0';
}

export async function graphqlHttpInteraction(
  interaction: GraphqlUnconfiguredInteraction,
  options: GraphqlRequestOptions & { response: unknown },
): Promise<GraphqlInteractionWithPluginResponse>;
export async function graphqlHttpInteraction(
  interaction: GraphqlUnconfiguredInteraction,
  options: GraphqlRequestOptions,
): Promise<GraphqlInteractionWithPluginRequest>;
export async function graphqlHttpInteraction(
  interaction: GraphqlUnconfiguredInteraction,
  options: GraphqlRequestOptions,
): Promise<GraphqlInteractionWithPluginRequest | GraphqlInteractionWithPluginResponse> {
  const configuration = buildGraphqlConfiguration(options);

  const pluginInteraction = interaction.usingPlugin({
    plugin: 'graphql',
    version: pluginVersion(),
  });

  const withRequestResult = pluginInteraction.withRequest(
    'POST',
    '/graphql',
    (builder: GraphqlRequestWithPluginBuilder) => {
      builder.headers({ 'content-type': 'application/graphql' });
      builder.pluginContents('application/graphql', JSON.stringify(configuration));
    },
  );

  if (options.response === undefined) {
    return withRequestResult;
  }

  // The response-side configure call is a *separate* invocation that shares no state with the
  // request-side one, so it must carry the same canonical query/operation/schema fields itself —
  // otherwise selection-set validation would compare against the wrong document.
  const responseConfiguration = {
    query_document: configuration.query_document,
    operation_name: configuration.operation_name,
    // Carried so the plugin sees the operation's declared variables as satisfied; without it the
    // whole interaction is rejected for an unsatisfied `$id: ID!`.
    variables_json: configuration.variables_json,
    schema_sdl: configuration.schema_sdl,
    response_body_json: JSON.stringify(options.response),
  };

  return withRequestResult.willRespondWith(
    options.status ?? 200,
    (builder: GraphqlResponseWithPluginBuilder) => {
      // Set BEFORE pluginContents: without an explicit content-type header, pact_ffi's HTTP
      // callback falls back to the content type passed to the FFI call itself
      // (application/graphql-response), which is wrong on the wire.
      builder.headers({ 'content-type': 'application/json' });
      builder.pluginContents(
        GRAPHQL_RESPONSE_CONTENT_TYPE,
        JSON.stringify(responseConfiguration),
      );
    },
  );
}

export async function graphqlMessageInteraction<T = unknown>(
  pact: GraphqlMessagePactBuilder<T>,
  options: GraphqlMessageOptions,
): Promise<GraphqlAsyncMessageWithPluginContents<T>> {
  const configuration = buildGraphqlConfiguration({
    schema: options.schema,
    query: options.subscription,
    operationName: options.operationName,
    variables: options.variables,
  });

  const envelopeVariables = buildEnvelopeVariables(options.variables);

  const envelope: Record<string, unknown> = {
    subscription: options.operationName,
    data: options.data,
  };

  if (envelopeVariables !== undefined) {
    envelope.variables = envelopeVariables;
  }

  const interaction = pact.addAsynchronousInteraction();
  // `usingPlugin` only loads the plugin; the configuration reaches it via `withPluginContents`
  // below. pact-js's `PluginConfig` has no `configuration` field, so passing one here was a no-op.
  const pluginInteraction = interaction.usingPlugin({
    plugin: 'graphql',
    version: pluginVersion(),
  });
  const pluginContents = pluginInteraction.withPluginContents(
    JSON.stringify(configuration),
    'application/graphql',
  );

  interaction.expectsToReceive('a GraphQL subscription event', (builder: GraphqlAsyncMessageBuilder) => {
    builder.withJSONContent(envelope);
  });

  return pluginContents;
}

export { graphql, gql, DEFAULT_PLUGIN_VERSION } from './dsl';

export type {
  GraphqlApi,
  GraphqlApiOptions,
  GraphqlClient,
  GraphqlInteractionBuilder,
  GraphqlPactBuilder,
  GraphqlQueryMatching,
  FetchLike,
} from './types';

export type {
  GraphqlInteractionWithPlugin,
  GraphqlInteractionWithPluginRequest,
  GraphqlInteractionWithPluginResponse,
  GraphqlUnconfiguredInteraction,
  GraphqlMessageOptions,
  GraphqlMessagePactBuilder,
  GraphqlAsyncMessageWithPluginContents,
  GraphqlRequestOptions,
  GraphqlTransport,
} from './types';
