import 'source-map-support/register';

import { setupCommands, setupIngressCommands } from './command';

async function bootstrap() {
  await (
    process.env.ADC_RUNNING_MODE === 'ingress'
      ? setupIngressCommands()
      : setupCommands()
  ).parseAsync(process.argv);
}

bootstrap();
