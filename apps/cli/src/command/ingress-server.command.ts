import { ADCServer } from '../server';
import { BaseCommand } from './helper';

export const IngressServerCommand = new BaseCommand('server').handle(
  async () => {
    await new ADCServer().start();
    console.log('ADC server is running on: http://127.0.0.1:3000');
  },
);
