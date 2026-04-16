import { getAsset, isSea } from 'node:sea';
import sourceMapSupport from 'source-map-support';

import { setupCommands, setupIngressCommands } from './command';

function setupSourceMap() {
  if (!isSea()) {
    sourceMapSupport.install();
    return;
  }

  try {
    const sourceMapFile = getAsset('main.js.map', 'utf-8');
    const map = JSON.parse(sourceMapFile);
    sourceMapSupport.install({
      environment: 'node',
      retrieveSourceMap: (source) => {
        if (source.startsWith('node:') || source.startsWith('internal'))
          return null;
        return { url: source, map };
      },
    });
  } catch (err) {
    console.warn(
      `Failed to load source map for SEA: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
}

async function bootstrap() {
  setupSourceMap();

  await (
    process.env.ADC_RUNNING_MODE === 'ingress'
      ? setupIngressCommands()
      : setupCommands()
  ).parseAsync(process.argv);
}

bootstrap();
