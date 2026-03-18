import pino, { type Logger, stdTimeFunctions } from "pino";

const logger: Logger =
  process.env.NODE_ENV === "production"
    ? pino({
        level: "info",
        base: { app: "symphony" },
        timestamp: stdTimeFunctions.isoTime,
        redact: {
          paths: [
            "password",
            "headers.authorization",
            "headers.cookie",
            "cookies",
            "token",
          ],
          remove: false,
        },
      })
    : pino({
        level: "debug",
        base: { app: "symphony" },
        timestamp: stdTimeFunctions.isoTime,
        transport: {
          target: "pino-pretty",
          options: {
            colorize: true,
            translateTime: "SYS:standard",
            ignore: "pid,hostname",
            singleLine: false,
          },
        },
      });

export function createModuleLogger(moduleName: string): Logger {
  return logger.child({ module: moduleName });
}
