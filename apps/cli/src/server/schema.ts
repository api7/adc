import { z } from 'zod';

const SyncTask = z.strictObject({
  opts: z.looseObject({
    backend: z.union([
      z.literal('apisix'),
      z.literal('api7ee'),
      z.literal('apisix-standalone'),
    ]),
    server: z.url().min(1),
    token: z.string().min(1),
    lint: z.boolean().optional().default(true),
  }),
  config: z.looseObject({}),
});

export const SyncInput = z.strictObject({
  task: SyncTask,
});
export type SyncInputType = z.infer<typeof SyncInput>;
