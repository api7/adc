import type { PluginSchemaMap } from '@api7/adc-sdk';

export interface LintError {
  path: Array<string | number>;
  message: string;
  code: string;
  [key: string]: unknown;
}

export interface LintResult {
  success: boolean;
  errors: LintError[];
  data?: unknown;
}

export interface LintOptions {
  pluginSchemas?: PluginSchemaMap;
}
