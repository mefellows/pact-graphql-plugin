import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // Consumer test files here all write to the same pact file. Vitest runs test files in
    // parallel by default, and concurrent writers clobber each other's interactions — running the
    // whole suite produced a pact missing interactions that were present when the files were run
    // individually. Pact accumulates interactions per file, so the writes must be serialised.
    fileParallelism: false,

    // Provider verification consumes the pacts the consumer tests produce, so it cannot run in the
    // same pass: vitest does not order files alphabetically, and verification raced ahead of the
    // test generating its input. `npm test` writes the pacts; `npm run verify` verifies them.
    exclude: ['**/node_modules/**', '**/verify-provider.test.ts'],
  },
});
