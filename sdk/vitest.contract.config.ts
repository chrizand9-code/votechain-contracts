import { defineConfig } from "vitest/config";

// Separate config for consumer-driven contract tests (issue #308), kept out
// of the default `vitest.config.ts` include pattern (`src/**/*.test.ts`) so
// that `npm test` does not require `@pact-foundation/pact` to be installed.
// Run explicitly with `npm run test:contract`.
export default defineConfig({
  test: {
    globals: true,
    environment: "node",
    include: ["src/**/*.pact.spec.ts"],
  },
});
