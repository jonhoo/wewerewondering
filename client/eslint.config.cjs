const globals = require("globals");
const js = require("@eslint/js");
const eslintConfigPrettier = require("eslint-config-prettier");
const eslintPluginSvelte = require("eslint-plugin-svelte");

module.exports = [
	js.configs.recommended,
	eslintConfigPrettier,
	...eslintPluginSvelte.configs["flat/recommended"],
	...eslintPluginSvelte.configs["flat/prettier"],
	{
		ignores: ["dist/**"]
	},
	{
		languageOptions: {
			ecmaVersion: 2024,
			sourceType: "module",
			globals: {
				...globals.browser,
				...globals.node // for require
			}
		}
	},
	{
		rules: {
			"no-unused-vars": ["error", { varsIgnorePattern: "^_", argsIgnorePattern: "^_" }]
		}
	}
];
