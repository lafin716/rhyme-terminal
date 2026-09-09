import js from "@eslint/js";
import tseslint from "typescript-eslint";
import vue from "eslint-plugin-vue";

export default tseslint.config(
  { ignores: ["dist/**", "dist-mobile/**", "src-tauri/**", ".scratch/**"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...vue.configs["flat/essential"],
  {
    files: ["src/**/*.{ts,vue}"],
    languageOptions: {
      parserOptions: { parser: tseslint.parser },
    },
    rules: {
      // TypeScript already checks unused locals/parameters. Existing bridge
      // and test mocks intentionally use any; avoid a style migration in CI.
      "no-undef": "off",
      "@typescript-eslint/no-unused-vars": "off",
      "@typescript-eslint/no-explicit-any": "off",
      // PaneTabs updates fields on the shared reactive layout node.
      "vue/no-mutating-props": ["error", { shallowOnly: true }],
      "vue/multi-word-component-names": ["error", { ignores: ["Terminal"] }],
    },
  },
  {
    files: ["src/vite-env.d.ts"],
    rules: {
      "@typescript-eslint/no-empty-object-type": ["error", { allowObjectTypes: "always" }],
    },
  },
  {
    files: ["src/composables/useResources.ts"],
    // URL accessors can normalize values when assigned to themselves.
    rules: { "no-self-assign": ["error", { props: false }] },
  },
);
