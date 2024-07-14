import { setupCommands } from './command';

async function bootstrap() {
  await setupCommands().parseAsync(process.argv);
}

bootstrap();
