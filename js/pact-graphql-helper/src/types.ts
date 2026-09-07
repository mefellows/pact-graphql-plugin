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

/**
 * Structural mirrors of pact-js's V4 plugin interaction chain
 * (`@pact-foundation/pact/src/v4/http/types`). Those types are not re-exported from the package
 * root, and deep-importing `src/` would couple this helper to pact-js's internal layout, so they
 * are mirrored here instead.
 *
 * The states matter and must not be collapsed: `pluginContents` exists only on the builders
 * reached *after* `usingPlugin`. An earlier version of this file put `pluginContents` on the base
 * request builder, which no pact-js type has, so every call site failed to typecheck.
 */
export interface PluginInvocationOptions {
  plugin: string;
  version: string;
}

/**
 * Mirrors `V4RequestWithPluginBuilder`. The full member set is reproduced, not just the two this
 * helper calls, so a caller passing its own builder function is not artificially restricted.
 */
export interface GraphqlRequestWithPluginBuilder {
  query(query: Record<string, unknown>): unknown;
  headers(headers: Record<string, unknown>): unknown;
  jsonBody(body: unknown): unknown;
  xmlBody(body: unknown): unknown;
  binaryFile(contentType: string, file: string): unknown;
  multipartBody(
    contentType: string,
    filename: string,
    mimePartName: string,
    boundary?: string,
  ): unknown;
  body(contentType: string, body: Buffer): unknown;
  matchingRules(rules: unknown): unknown;
  pluginContents(contentType: string, contents: string): unknown;
}

/** Mirrors `V4ResponseWithPluginBuilder`. */
export interface GraphqlResponseWithPluginBuilder {
  headers(headers: Record<string, unknown>): unknown;
  jsonBody(body: unknown): unknown;
  xmlBody(body: unknown): unknown;
  binaryFile(contentType: string, file: string): unknown;
  multipartBody(
    contentType: string,
    filename: string,
    mimePartName: string,
    boundary?: string,
  ): unknown;
  body(contentType: string, body: Buffer): unknown;
  matchingRules(rules: unknown): unknown;
  pluginContents(contentType: string, contents: string): unknown;
}

/** Mirrors `V4InteractionWithPluginResponse`. */
export interface GraphqlInteractionWithPluginResponse {
  executeTest<T>(
    testFn: (mockServer: { url: string }) => Promise<T>,
  ): Promise<T | undefined>;
}

/** Mirrors `V4InteractionWithPluginRequest`. */
export interface GraphqlInteractionWithPluginRequest {
  willRespondWith(
    status: number,
    builder?: (builder: GraphqlResponseWithPluginBuilder) => void,
  ): GraphqlInteractionWithPluginResponse;
}

/** Mirrors `V4InteractionWithPlugin`. */
export interface GraphqlInteractionWithPlugin {
  withRequest(
    method: string,
    path: string,
    builder?: (builder: GraphqlRequestWithPluginBuilder) => void,
  ): GraphqlInteractionWithPluginRequest;
}

/** Mirrors `V4UnconfiguredInteraction`, narrowed to what this helper drives. */
export interface GraphqlUnconfiguredInteraction {
  given(state: string, parameters?: Record<string, unknown>): GraphqlUnconfiguredInteraction;
  uponReceiving(description: string): GraphqlUnconfiguredInteraction;
  usingPlugin(config: PluginInvocationOptions): GraphqlInteractionWithPlugin;
}

/**
 * Async message chain. Mirrors pact-js's V4 message types, which are a separate builder chain from
 * the HTTP one above.
 */
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

export type GraphqlQueryMatching = 'exact' | 'semantic' | 'subset';

export interface FetchLike {
  (url: string, init: { method: string; headers: Record<string, string>; body: string }): Promise<{
    json(): Promise<unknown>;
  }>;
}

export interface GraphqlApiOptions {
  /** SDL for the API under test. Supplied once per pact rather than per interaction. */
  schema?: string;
  /** Path the GraphQL endpoint is served from. Defaults to `/graphql`. */
  path?: string;
  transport?: GraphqlTransport;
  /** Pins the plugin version. Defaults to `PACT_GRAPHQL_PLUGIN_VERSION`, then the built-in. */
  pluginVersion?: string;
  /** Injectable for tests; defaults to the global `fetch`. */
  fetch?: FetchLike;
}

/** A client bound to the mock server, the configured path, and this interaction's document. */
export interface GraphqlClient {
  url: string;
  execute<TData = any>(overrides?: {
    variables?: unknown;
    operationName?: string;
  }): Promise<TData>;
}

export interface GraphqlInteractionBuilder {
  given(state: string): this;
  query(document: string): this;
  operationName(name: string): this;
  variables(variables: unknown): this;
  matching(mode: GraphqlQueryMatching): this;
  willRespondWith(body: unknown, status?: number): this;
  /** Configures the interaction without running it. Rejects if the plugin rejects it. */
  build(): Promise<GraphqlInteractionWithPluginRequest | GraphqlInteractionWithPluginResponse>;
  /** Returns `T | undefined`, matching pact-js's own `executeTest` signature. */
  executeTest<T>(handler: (client: GraphqlClient) => Promise<T>): Promise<T | undefined>;
}

export interface GraphqlApi {
  interaction(description: string): GraphqlInteractionBuilder;
}

export interface GraphqlPactBuilder {
  addInteraction(): GraphqlUnconfiguredInteraction;
}
