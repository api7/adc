import { OpenAPIConverter } from '@api7/adc-converter-openapi';
import * as ADCSDK from '@api7/adc-sdk';
import OpenAPIParser from '@readme/openapi-parser';
import { dump } from 'js-yaml';
import { Listr } from 'listr2';
import { cloneDeep } from 'lodash';
import { existsSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { OpenAPIV3 } from 'openapi-types';

import { SignaleRenderer } from '../utils/listr';
import { TaskContext } from './diff.command';
import { BaseCommand } from './helper';

interface ConvertOptions {
  file: Array<string>;
  output: string;
  verbose: number;
}

type ConvertContext = TaskContext & {
  oas?: OpenAPIV3.Document;
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
              // check existance
              if (!existsSync(filePath)) {
                const error = new Error(
                  `File "${resolve(filePath)}" does not exist`,
                );
                error.stack = '';
                throw error;
              }

              try {
                const oas = (await OpenAPIParser.dereference(
                  filePath,
                )) as OpenAPIV3.Document;
                const task = new OpenAPIConverter().toADC(oas);
                task.add([
                  {
                    task: (subCtx) => {
                      if (!ctx.buffer) {
                        ctx.buffer = [cloneDeep(subCtx.local)];
                      } else {
                        ctx.buffer.push(cloneDeep(subCtx.local));
                      }
                    },
                  },
                ]);
                return task;
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
  .description(
    'Convert API definitions in other formats to equivalent ADC configuration.',
  )
  .summary('convert API definitions in other formats to ADC configuration')
  .addCommand(OpenAPICommand);
