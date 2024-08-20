import { OpenAPIConverter } from '@api7/adc-converter-openapi';
import OpenAPIParser from '@readme/openapi-parser';
import { Listr } from 'listr2';
import { existsSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { OpenAPIV3 } from 'openapi-types';
import { stringify } from 'yaml';

import { SignaleRenderer } from '../utils/listr';
import { TaskContext } from './diff.command';
import { BaseCommand } from './helper';

interface ConvertOptions {
  file: string;
  output: string;
  verbose: number;
}

class BaseConvertCommand extends BaseCommand {
  constructor(name: string) {
    super(name);
    this.option(
      '-f, --file <openapi-file-path>',
      'OpenAPI specification file path',
    ).option('-o, --output <output-path>', 'output file path');
  }
}

const OpenAPICommand = new BaseConvertCommand('openapi')
  .description('Convert an OpenAPI specification to equivalent ADC configuration.\n\nLearn more at: https://docs.api7.ai/enterprise/reference/openapi-adc')
  .summary('convert OpenAPI spec to ADC configuration')
  .action(async () => {
    const opts = OpenAPICommand.optsWithGlobals<ConvertOptions>();

    const tasks = new Listr<
      TaskContext & { oas?: OpenAPIV3.Document },
      typeof SignaleRenderer
    >(
      [
        {
          title: 'Load OpenAPI document',
          task: async (ctx) => {
            const filePath = opts.file;

            // check existance
            if (!existsSync(filePath)) {
              const error = new Error(
                `File "${resolve(filePath)}" does not exist`,
              );
              error.stack = '';
              throw error;
            }

            try {
              ctx.oas = (await OpenAPIParser.dereference(
                filePath,
              )) as OpenAPIV3.Document;
            } catch (error) {
              error.message = error.message.replace('\n', '');
              error.stack = '';
              throw error;
            }
          },
        },
        {
          title: 'Convert OpenAPI document',
          task: (ctx) => new OpenAPIConverter().toADC(ctx.oas),
        },
        {
          title: 'Write converted OpenAPI file',
          task: (ctx, task) => {
            const yamlStr = stringify(ctx.local, {});
            if (!opts.output) opts.output = './adc.yaml';
            writeFileSync(opts.output, yamlStr);
            task.title = `Converted OpenAPI file to "${resolve(
              opts.output,
            )}" successfully`;
          },
        },
      ],
      {
        renderer: SignaleRenderer,
        rendererOptions: { verbose: opts.verbose },
        ctx: { local: {} },
      },
    );

    try {
      await tasks.run();
    } catch (err) {
      /* ignore */
    }
  });

export const ConvertCommand = new BaseCommand('convert')
  .description('Convert API definitions in other formats to equivalent ADC configuration.')
  .summary('convert API definitions in other formats to ADC configuration')
  .addCommand(OpenAPICommand);
