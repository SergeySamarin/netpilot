import { defineConfig, globalIgnores } from 'eslint/config';
import globals from 'globals';
import tsEslintPlugin from '@typescript-eslint/eslint-plugin';
import tsEslintParser from '@typescript-eslint/parser';
import pluginReact from 'eslint-plugin-react';
import pluginReactHooks from 'eslint-plugin-react-hooks';

export default defineConfig([
   globalIgnores(['dist/', 'node_modules/', 'src-tauri/target/', 'src-tauri/gen/', 'design/']),
   {
      linterOptions: {
         reportUnusedDisableDirectives: false
      },
      extends: [tsEslintPlugin.configs['flat/disable-type-checked']],
      languageOptions: {
         ecmaVersion: 2022,
         sourceType: 'module',
         globals: {
            ...globals.browser,
            ...globals.node
         }
      },
      plugins: {
         '@typescript-eslint': tsEslintPlugin,
         react: pluginReact,
         'react-hooks': pluginReactHooks
      },
      rules: {
         'no-debugger': 'error',
         'no-empty': 'error',
         'no-unsafe-finally': 'error',
         'use-isnan': 'error',

         'dot-notation': ['error', { allowKeywords: true }],
         eqeqeq: ['error', 'always', { null: 'ignore' }],
         'guard-for-in': 'error',
         'no-caller': 'error',
         'no-eval': 'error',
         'no-new-wrappers': 'error',
         'no-return-await': 'error',
         'no-sequences': 'error',
         radix: 'error',

         'no-undef-init': 'error',

         'constructor-super': 'error'
      }
   },
   {
      files: ['**/*.ts', '**/*.tsx'],
      languageOptions: {
         parser: tsEslintParser
      },
      rules: {
         '@typescript-eslint/adjacent-overload-signatures': 'error',
         '@typescript-eslint/array-type': ['error', { default: 'array' }],
         '@typescript-eslint/prefer-function-type': 'error',
         'no-labels': ['error', { allowLoop: true, allowSwitch: true }],
         'max-classes-per-file': ['error', 7],
         '@typescript-eslint/consistent-type-assertions': [
            'warn',
            { assertionStyle: 'as', objectLiteralTypeAssertions: 'allow' }
         ],
         '@typescript-eslint/no-explicit-any': 'warn',
         'no-bitwise': 'error',
         'no-cond-assign': ['warn', 'always'],
         'no-console': 'error',
         '@typescript-eslint/no-empty-interface': ['error', { allowSingleExtends: true }],
         '@typescript-eslint/no-misused-new': 'error',
         '@typescript-eslint/no-namespace': 'error',
         '@typescript-eslint/triple-slash-reference': [
            'error',
            { path: 'never', types: 'never', lib: 'never' }
         ],
         '@typescript-eslint/no-unused-expressions': 'error',
         'no-var': 'error',
         '@typescript-eslint/no-var-requires': 'error',
         'object-shorthand': 'error',
         'prefer-const': 'error',
         '@typescript-eslint/unified-signatures': 'error',
         'max-len': [
            'error',
            {
               code: 120,
               ignoreUrls: true,
               ignoreComments: true,
               ignoreRegExpLiterals: true,
               ignoreStrings: true,
               ignoreTemplateLiterals: true,
               ignorePattern: '^ +\\* '
            }
         ],
         '@typescript-eslint/explicit-member-accessibility': [
            'error',
            { accessibility: 'no-public' }
         ],
         '@typescript-eslint/no-magic-numbers': [
            'warn',
            {
               ignore: [-1, 0, 1, 2],
               ignoreEnums: true,
               ignoreNumericLiteralTypes: true,
               ignoreReadonlyClassProperties: true
            }
         ],
         '@typescript-eslint/no-non-null-assertion': 'error',
         'no-param-reassign': 'warn',
         '@typescript-eslint/no-this-alias': 'warn',
         'prefer-object-spread': 'error',
         'one-var': ['error', 'never'],
         '@typescript-eslint/typedef': [
            'warn',
            {
               parameter: true,
               propertyDeclaration: true,
               memberVariableDeclaration: true,
               variableDeclarationIgnoreFunction: true
            }
         ],
         '@typescript-eslint/ban-tslint-comment': 'error',
         'react-hooks/exhaustive-deps': 'warn'
      }
   },
   {
      files: ['**/*.tsx'],
      languageOptions: {
         parserOptions: {
            ecmaFeatures: { jsx: true }
         }
      },
      settings: {
         react: { version: '19.0' }
      },
      rules: {
         'react-hooks/rules-of-hooks': 'error',
         'react/function-component-definition': [
            'error',
            {
               namedComponents: ['function-declaration', 'function-expression'],
               unnamedComponents: 'function-expression'
            }
         ],
         'react/jsx-max-props-per-line': ['error', { maximum: 1, when: 'multiline' }],
         'react/jsx-no-duplicate-props': ['error', { ignoreCase: true }],
         'react/no-danger-with-children': 'error',
         'react/no-array-index-key': 'error',
         'react/jsx-fragments': ['error', 'syntax'],
         'react/jsx-no-useless-fragment': 'error',
         'react/jsx-no-constructed-context-values': 'error',
         'react/no-unstable-nested-components': ['error', { allowAsProps: true }],
         'react/hook-use-state': 'error'
      }
   }
]);
