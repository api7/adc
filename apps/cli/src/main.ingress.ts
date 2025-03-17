import { setupIngressCommands } from './command';

async function bootstrap() {
  await setupIngressCommands().parseAsync(process.argv);
}

bootstrap();
