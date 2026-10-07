import { appJa } from "./app";
import { authoringJa } from "./authoring";
import { demoJa } from "./demo";
import { errorsJa } from "./errors";
import { executionJa } from "./execution";
import { metaJa } from "./meta";
import { recoveryJa } from "./recovery";
import { traceJa } from "./trace";

export const jaJP = {
  ...metaJa,
  app: appJa,
  authoring: authoringJa,
  execution: executionJa,
  recovery: recoveryJa,
  trace: traceJa,
  errors: errorsJa,
  demo: demoJa,
} as const;

export type JaJpCatalog = typeof jaJP;
