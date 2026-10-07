import { appJa } from "./app.ts";
import { authoringJa } from "./authoring.ts";
import { demoJa } from "./demo.ts";
import { errorsJa } from "./errors.ts";
import { executionJa } from "./execution.ts";
import { metaJa } from "./meta.ts";
import { recoveryJa } from "./recovery.ts";
import { traceJa } from "./trace.ts";

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
