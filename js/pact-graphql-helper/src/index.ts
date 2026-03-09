import type {
  GraphqlHttpInteractionBuilder,
  GraphqlHttpRequestBuilder,
  GraphqlMessageOptions,
  GraphqlMessagePactBuilder,
  GraphqlRequestOptions,
  GraphqlTransport,
  PluginCapableInteractionBuilder,
} from './types';

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

export async function graphqlInteraction<T = unknown>(
  builder: PluginCapableInteractionBuilder<T>,
  options: GraphqlRequestOptions,
): Promise<T> {
  const configuration = buildGraphqlConfiguration(options);

  const version = process.env.PACT_GRAPHQL_PLUGIN_VERSION ?? '0.0.0';

  return builder.usingPlugin({
    plugin: 'graphql',
    version,
    configuration,
  });
}

export async function graphqlHttpInteraction<T = unknown>(
  interaction: GraphqlHttpInteractionBuilder<T> &
    PluginCapableInteractionBuilder<GraphqlHttpInteractionBuilder<T>>,
  options: GraphqlRequestOptions,
): Promise<T> {
  const pluginInteraction = await graphqlInteraction(interaction, options);
  const configuration = buildGraphqlConfiguration(options);

  return pluginInteraction.withRequest(
    'POST',
    '/graphql',
    (builder: GraphqlHttpRequestBuilder) => {
      builder.headers({ 'content-type': 'application/graphql' });
      builder.pluginContents('application/graphql', JSON.stringify(configuration));
    },
  );
}

export async function graphqlMessageInteraction<T = unknown>(
  pact: GraphqlMessagePactBuilder<T>,
  options: GraphqlMessageOptions,
): Promise<T> {
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

  const interaction = pact.addAsyncMessage();
  interaction.pluginContents('application/graphql', JSON.stringify(configuration));
  return interaction.withContents('application/json', envelope);
}

export type {
  GraphqlHttpInteractionBuilder,
  GraphqlMessageInteractionBuilder,
  GraphqlMessageOptions,
  GraphqlMessagePactBuilder,
  GraphqlRequestOptions,
  GraphqlTransport,
  PluginCapableInteractionBuilder,
} from './types';
