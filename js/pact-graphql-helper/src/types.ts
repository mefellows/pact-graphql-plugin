export type GraphqlTransport = 'json_body' | 'query_string';

export interface GraphqlRequestOptions {
  schema?: string;
  query: string;
  operationName?: string;
  variables?: unknown;
  transport?: GraphqlTransport;
}

export interface PluginInvocationOptions {
  plugin: string;
  version: string;
  configuration: Record<string, unknown>;
}

export interface PluginCapableInteractionBuilder<T = unknown> {
  usingPlugin(options: PluginInvocationOptions): Promise<T> | T;
}
