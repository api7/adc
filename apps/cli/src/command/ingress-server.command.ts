import { ADCServer } from '../server';
import { BaseCommand } from './helper';

export const IngressServerCommand = new BaseCommand('server').handle(() =>
  new ADCServer().start(),
);
