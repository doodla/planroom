import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  {
    // Generated files are checked by the drift checks; examples, docs and .claude hold no TypeScript.
    ignores: [
      "**/node_modules/",
      "target/",
      "schema/",
      "packages/contract/src/generated/",
      "packages/mod/vendor/",
      "plugin/hooks/planroom-mod.js",
      "plugin/types/",
      ".dependency-cruiser.cjs",
      "examples/",
      "docs/",
      ".claude/",
    ],
  },
  js.configs.recommended,
  tseslint.configs.recommendedTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    // Over the mod API's hook and element unions (packages/mod/vendor/claude-code.d.ts) this one
    // rule takes more than 10 minutes a file; tsc -b still type-checks these files, and the host
    // awaits a promise a handler returns.
    files: ["packages/mod/src/register.tsx", "packages/mod/plugin-tests/**"],
    rules: { "@typescript-eslint/no-misused-promises": "off" },
  },
  {
    files: ["**/*.js", "**/*.cjs", "**/*.mjs"],
    extends: [tseslint.configs.disableTypeChecked],
  },
);
