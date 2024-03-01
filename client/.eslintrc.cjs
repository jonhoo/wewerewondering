module.exports = {
	root: true,
	extends: ["eslint:recommended", "prettier", "plugin:svelte/prettier"],
	plugins: [],
	parserOptions: {
		sourceType: "module",
		ecmaVersion: 2024
	},
	env: {
		browser: true,
		es2024: true
	},
	rules: {
		"no-unused-vars": ["error", { varsIgnorePattern: "^_", argsIgnorePattern: "^_" }]
	}
};
