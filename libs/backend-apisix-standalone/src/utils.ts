export const stableTimestamp = () =>
  Math.floor(performance.timeOrigin + performance.now());
