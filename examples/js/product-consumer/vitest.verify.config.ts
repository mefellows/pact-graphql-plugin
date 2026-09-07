import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // Verification runs on its own, after `npm test` has written the pacts. Kept separate from
    // vitest.config.ts, which excludes this file so it cannot race the tests that generate its
    // input.
    fileParallelism: false,
    include: ['**/verify-provider.test.ts'],
  },
});
