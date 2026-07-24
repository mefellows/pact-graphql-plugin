export type GraphqlTransport = 'json_body' | 'query_string';

export interface GraphqlRequestOptions {
  schema?: string;
  query: string;
  operationName?: string;
  variables?: unknown;
  transport?: GraphqlTransport;
  /** Expected GraphQL response envelope, e.g. `{ data: { product: {...} } }`. */
  response?: unknown;
  /** HTTP status for the expected response. Defaults to 200. */
  status?: number;
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

export interface GraphqlAsyncMessageBuilder {
  withJSONContent(content: unknown): GraphqlAsyncMessageBuilder;
}

export interface GraphqlAsyncMessageWithContent<T = unknown> {
  executeTest(handler: (message: { contents: { content: unknown } }) => Promise<T>):
    | Promise<T>
    | Promise<T | undefined>;
}

export interface GraphqlAsyncMessageWithPluginContents<T = unknown> {
  executeTest(handler: (message: { contents: { content: unknown } }) => Promise<T>):
    | Promise<T>
    | Promise<T | undefined>;
}

export interface GraphqlAsyncMessageWithPlugin<T = unknown> {
  expectsToReceive(description: string): GraphqlAsyncMessageWithPlugin<T>;
  withPluginContents(contents: string, contentType: string): GraphqlAsyncMessageWithPluginContents<T>;
}

export interface GraphqlUnconfiguredAsyncMessage<T = unknown> {
  expectsToReceive(description: string, builder: (builder: GraphqlAsyncMessageBuilder) => void):
    GraphqlAsyncMessageWithContent<T>;
  usingPlugin(options: PluginInvocationOptions): GraphqlAsyncMessageWithPlugin<T>;
}

export interface GraphqlMessagePactBuilder<T = unknown> {
  addAsynchronousInteraction(): GraphqlUnconfiguredAsyncMessage<T>;
}

export interface GraphqlHttpRequestBuilder {
  headers(headers: Record<string, string>): void;
  pluginContents(contentType: string, contents: string): void;
}

export interface GraphqlHttpResponseBuilder {
  headers(headers: Record<string, string>): void;
  pluginContents(contentType: string, contents: string): void;
}

/**
 * The value returned by `withRequest`: lets the caller (or this helper, internally, when
 * `GraphqlRequestOptions.response` is supplied) configure the response part via the plugin.
 */
export interface GraphqlHttpInteractionWithRequest<T = unknown> {
  willRespondWith(
    status: number,
    builder: (builder: GraphqlHttpResponseBuilder) => void,
  ): T;
}

export interface GraphqlHttpInteractionBuilder<T = unknown> {
  withRequest(
    method: string,
    path: string,
    builder: (builder: GraphqlHttpRequestBuilder) => void,
  ): T & GraphqlHttpInteractionWithRequest<T>;
}
