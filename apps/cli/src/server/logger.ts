import { type RequestHandler } from 'express';
import { randomUUID } from 'node:crypto';
import winston from 'winston';

declare global {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace Express {
    interface Request {
      requestId: string;
    }
    interface Response {
      responseBodyForLog: string;
    }
  }
}

export const logger = winston.createLogger({
  level: process.env.ADC_INGRESS_LOG_LEVEL ?? 'info',
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.json(),
  ),
  transports: [new winston.transports.Console({})],
});

// task.opts.tlsClientKey carries a raw mTLS private key; never let it reach
// the debug request-body log
const redactRequestBody = (body: unknown) => {
  const opts = (body as { task?: { opts?: { tlsClientKey?: unknown } } })?.task
    ?.opts;
  if (!opts || !('tlsClientKey' in opts)) return body;
  const { task, ...rest } = body as { task: { opts: object } };
  return { ...rest, task: { ...task, opts: { ...task.opts, tlsClientKey: '***' } } };
};

export const loggerMiddleware: RequestHandler = (req, res, next) => {
  req.requestId = randomUUID();

  logger.log({
    level: 'info',
    message: `${req.method} ${req.url}`,
    requestId: req.requestId,
  });

  if (req.body)
    logger.log({
      level: 'debug',
      message: '',
      requestBody: redactRequestBody(req.body),
      requestId: req.requestId,
    });

  next();
};
