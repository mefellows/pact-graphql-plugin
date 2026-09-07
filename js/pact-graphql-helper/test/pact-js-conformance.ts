/**
 * Compile-time conformance check between this helper's structural types and pact-js's real ones.
 *
 * Why this file exists: the helper mirrors pact-js's V4 plugin interaction chain structurally
 * rather than importing it, because those types are not re-exported from the package root and
 * deep-importing `src/` from the *published* surface would couple every consumer to pact-js's
 * internal layout. But an unchecked mirror drifts — an earlier one collapsed the plugin/non-plugin
 * builder states and hid three behavioural bugs (a silently-dropped `configuration` argument, a
 * needlessly awaited synchronous call, and a wrong return type).
 *
 * So the deep import lives here, in a dev-only file that consumers never resolve. If pact-js
 * changes the chain — or adds an `exports` map that hides this path — `npm run typecheck` fails
 * loudly in this repo, and only in this repo.
 *
 * This file is intentionally type-only: it emits nothing and runs nothing.
 */
import type {
  V4InteractionWithPlugin,
  V4InteractionWithPluginRequest,
  V4InteractionWithPluginResponse,
  V4RequestWithPluginBuilder,
  V4ResponseWithPluginBuilder,
  V4UnconfiguredInteraction,
} from '@pact-foundation/pact/src/v4/http/types';

import type {
  GraphqlInteractionWithPlugin,
  GraphqlInteractionWithPluginRequest,
  GraphqlInteractionWithPluginResponse,
  GraphqlRequestWithPluginBuilder,
  GraphqlResponseWithPluginBuilder,
  GraphqlUnconfiguredInteraction,
} from '../src/types';

/**
 * Assignability must hold in this direction: callers hand us pact-js's real objects, and our
 * functions declare the mirrored types, so the real type must satisfy the mirror.
 */
type MustAccept<Mirror, Real extends Mirror> = Real;

type _Unconfigured = MustAccept<GraphqlUnconfiguredInteraction, V4UnconfiguredInteraction>;
type _WithPlugin = MustAccept<GraphqlInteractionWithPlugin, V4InteractionWithPlugin>;
type _WithRequest = MustAccept<GraphqlInteractionWithPluginRequest, V4InteractionWithPluginRequest>;
type _WithResponse = MustAccept<
  GraphqlInteractionWithPluginResponse,
  V4InteractionWithPluginResponse
>;

/**
 * Builders travel the other way — pact-js hands *us* the builder inside its callback — so our
 * mirror must be satisfied by the real builder here too.
 */
type _RequestBuilder = MustAccept<GraphqlRequestWithPluginBuilder, V4RequestWithPluginBuilder>;
type _ResponseBuilder = MustAccept<GraphqlResponseWithPluginBuilder, V4ResponseWithPluginBuilder>;

export type ConformanceChecked = [
  _Unconfigured,
  _WithPlugin,
  _WithRequest,
  _WithResponse,
  _RequestBuilder,
  _ResponseBuilder,
];
