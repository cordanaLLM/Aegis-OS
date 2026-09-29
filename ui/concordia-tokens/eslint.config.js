// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// HISS for the JavaScript and Svelte under ui/ (D100), which is this package
// today: ui/concordia-tokens is the one UI package, so the lint's
// configuration, its local rule and its planted violations live in it and
// its packages come from its own lockfile. praetorctl audit has no
// JavaScript, TypeScript or Svelte scanner (cordanaLLM/praetor#589): a
// planted recursion, an eval and a 73-line function all pass it. Until that
// scanner ships, this configuration enforces the same limits here, offline,
// inside the pinned accessibility container (tools/verify_a11y.py), with zero
// warnings allowed:
//
//   HISS-04  a function is at most 60 lines, cyclomatic complexity 10 and 50
//            statements;
//   HISS-08  no eval, no string passed to setTimeout/setInterval, no Function
//            constructor;
//   HISS-01  no function calls itself by name (a local rule). Mutual
//            recursion, a cycle through two or more functions, is NOT detected:
//            there is no call-graph check here.
//
// Inline configuration is switched off, so an eslint-disable comment cannot
// turn a finding off; it is itself reported, and --max-warnings 0 fails on it.
import svelte from 'eslint-plugin-svelte';
import noSelfRecursion from './hiss-lint/no-self-recursion.js';

const HISS = {
  maxLines: 60,
  maxComplexity: 10,
  maxStatements: 50,
};

const hiss = {
  meta: { name: 'aegis-hiss' },
  rules: { 'no-self-recursion': noSelfRecursion },
};

export default [
  {
    // The planted violations are linted one at a time, with --no-ignore, by the
    // gate's negative cases; they must not fail the workspace's own lint.
    ignores: ['**/node_modules/', '**/dist/', '**/test-output/', 'hiss-lint/plants/'],
  },
  {
    linterOptions: {
      noInlineConfig: true,
      reportUnusedDisableDirectives: 'error',
      reportUnusedInlineConfigs: 'error',
    },
  },
  ...svelte.configs.base,
  {
    files: ['**/*.js', '**/*.mjs', '**/*.cjs', '**/*.svelte', '**/*.svelte.js'],
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
      // no-implied-eval reports a string passed to setTimeout or setInterval
      // only when the callee resolves to a declared global, so the timer
      // functions and the global objects it looks them up through are declared
      // here instead of through another package.
      globals: {
        setTimeout: 'readonly',
        setInterval: 'readonly',
        execScript: 'readonly',
        window: 'readonly',
        self: 'readonly',
        global: 'readonly',
      },
    },
    plugins: { 'aegis-hiss': hiss },
    rules: {
      'max-lines-per-function': [
        'error',
        { max: HISS.maxLines, skipBlankLines: false, skipComments: false, IIFEs: true },
      ],
      complexity: ['error', { max: HISS.maxComplexity }],
      'max-statements': ['error', { max: HISS.maxStatements }],
      'no-eval': 'error',
      'no-implied-eval': 'error',
      'no-new-func': 'error',
      'aegis-hiss/no-self-recursion': 'error',
    },
  },
];
