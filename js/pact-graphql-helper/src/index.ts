import type {
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

function resolveTransport(transport?: GraphqlTransport): GraphqlTransport {
  return transport ?? 'json_body';
}

export async function graphqlInteraction<T = unknown>(
  builder: PluginCapableInteractionBuilder<T>,
  options: GraphqlRequestOptions,
): Promise<T> {
  if (!options.query || options.query.trim().length === 0) {
    throw new Error('GraphQL query is required');
  }

  const configuration = {
    query_document: normalizeQuery(options.query),
    operation_name: options.operationName,
    variables_json: serializeVariables(options.variables),
    transport: resolveTransport(options.transport),
    schema_sdl: options.schema,
  };

  return builder.usingPlugin({
    pluginName: 'graphql',
    configuration,
  });
}

export type { GraphqlRequestOptions, GraphqlTransport, PluginCapableInteractionBuilder } from './types';
