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
    files: ["**/*.js", "**/*.cjs", "**/*.mjs"],
    extends: [tseslint.configs.disableTypeChecked],
  },
);
