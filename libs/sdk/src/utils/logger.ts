import { AxiosResponse } from 'axios';

export interface LogEntry {
  message: string;
  [key: string]: unknown;
}

export interface LogEntryOptions {
  showLogEntry: (log: LogEntry) => boolean;
}

export interface Logger {
  log(message: string): void;
  debug(log: LogEntry, opts?: LogEntryOptions): void;
  axiosDebug(resp: AxiosResponse, desc?: string): void;
}
