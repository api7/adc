import { OpenAPIConverter } from '@api7/adc-converter-openapi';
import * as ADCSDK from '@api7/adc-sdk';
import { InvalidArgumentError } from 'commander';
import { dump } from 'js-yaml';
import { Listr } from 'listr2';
import { cloneDeep } from 'lodash';
import { existsSync, writeFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { lastValueFrom } from 'rxjs';

import { SignaleRenderer } from '../utils/listr';
import { TaskContext } from './diff.command';
import { BaseCommand } from './helper';

interface ConvertOptions {
  file: Array<string>;
  output: string;
  verbose: number;
}

type ConvertContext = TaskContext & {
  buffer?: Array<ADCSDK.Configuration>;
};

class BaseConvertCommand extends BaseCommand {
  constructor(name: string) {
    super(name);
    this.option(
      '-f, --file <openapi-file-path>',
      'OpenAPI specification file path',
      (filePath, files: Array<string> = []) => files.concat(filePath),
    ).option('-o, --output <output-path>', 'output file path', 'adc.yaml');
  }
}

const OpenAPICommand = new BaseConvertCommand('openapi')
  .description(
    'Convert an OpenAPI specification to equivalent ADC configuration.\n\nLearn more at: https://docs.api7.ai/enterprise/reference/openapi-adc',
  )
  .summary('convert OpenAPI spec to ADC configuration')
  .addExamples([
    {
      title:
        'Convert OpenAPI specification in YAML format to ADC configuration and write to the default adc.yaml file',
      command: 'adc convert openapi -f openapi.yaml',
    },
    {
      title:
        'Convert OpenAPI specification in JSON format to ADC configuration and write to the specified file',
      command: 'adc convert openapi -f openapi.json -o converted-adc.yaml',
    },
    {
      title:
        'Convert multiple OpenAPI specifications to single ADC configuration',
      command: 'adc convert openapi -f openapi.yaml -f openapi.json',
    },
  ])
  .action(async () => {
    const opts = OpenAPICommand.optsWithGlobals<ConvertOptions>();

    const tasks = new Listr<ConvertContext, typeof SignaleRenderer>(
      [
        ...opts.file.map((filePath) => {
          return {
            title: `Convert OpenAPI document "${resolve(filePath)}"`,
            task: async (ctx: ConvertContext) => {
              if (!existsSync(filePath))
                throw new InvalidArgumentError(
                  `File "${resolve(filePath)}" does not exist`,
                );

              try {
                const content = await readFile(filePath, 'utf-8');
                const config = await lastValueFrom(
                  new OpenAPIConverter().toADC(content),
                );
                ctx.buffer.push(cloneDeep(config));
              } catch (error) {
                error.message = error.message.replace('\n', '');
                error.stack = '';
                throw error;
              }
            },
          };
        }),
        {
          title: 'Write converted OpenAPI file',
          task: (ctx, task) => {
            ctx.local.services = ctx.buffer.flatMap((item) => item.services);
            const yamlStr = dump(ctx.local, { noRefs: true, sortKeys: true });
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
        ctx: { local: {}, buffer: [] },
      },
    );

    try {
      await tasks.run();
    } catch (err) {
      /* ignore */
    }
  });

export const ConvertCommand = new BaseCommand('convert')
  .description(
    'Convert API definitions in other formats to equivalent ADC configuration.',
  )
  .summary('convert API definitions in other formats to ADC configuration')
  .addCommand(OpenAPICommand);
