import { readFile } from 'fs/promises';
import YAML from 'js-yaml';

export const ExperimentalRemoteStateFileTask = (file: string) => ({
  task: async (ctx) => {
    const fileContent =
      (await readFile(file, {
        encoding: 'utf-8',
      })) ?? '';
    ctx.remote = YAML.load(fileContent);
  },
});
