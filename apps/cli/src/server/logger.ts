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
      message: undefined,
      requestBody: req.body,
      requestId: req.requestId,
    });

  next();
};
