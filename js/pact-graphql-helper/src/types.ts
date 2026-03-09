export type GraphqlTransport = 'json_body' | 'query_string';

export interface GraphqlRequestOptions {
  schema?: string;
  query: string;
  operationName?: string;
  variables?: unknown;
  transport?: GraphqlTransport;
}

export interface GraphqlMessageOptions {
  schema?: string;
  subscription: string;
  operationName: string;
  variables?: unknown;
  data: unknown;
}

export interface PluginInvocationOptions {
  plugin: string;
  version: string;
  configuration: Record<string, unknown>;
}

export interface PluginCapableInteractionBuilder<T = unknown> {
  usingPlugin(options: PluginInvocationOptions): Promise<T> | T;
}

export interface GraphqlMessageInteractionBuilder<T = unknown>
  extends PluginCapableInteractionBuilder<T> {
  pluginContents(contentType: string, contents: string): void;
  withContents(contents: unknown): T;
  withContents(contentType: string, contents: unknown): T;
}

export interface GraphqlMessagePactBuilder<T = unknown> {
  addAsyncMessage(): GraphqlMessageInteractionBuilder<T>;
}

export interface GraphqlHttpRequestBuilder {
  headers(headers: Record<string, string>): void;
  pluginContents(contentType: string, contents: string): void;
}

export interface GraphqlHttpInteractionBuilder<T = unknown> {
  withRequest(
    method: string,
    path: string,
    builder: (builder: GraphqlHttpRequestBuilder) => void,
  ): T;
}
