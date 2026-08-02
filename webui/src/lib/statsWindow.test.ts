// The test timezone is pinned to America/New_York in vite.config.ts so the
// DST case below actually crosses a transition (2026-03-08 is the
// spring-forward day there).

import { describe, expect, it } from "vitest";
import { computeStatsWindow } from "./statsWindow";

// All fixtures are built from local-time components so the tests hold in any
// timezone: "now" is 2026-08-02 15:30 local, and expected bounds are the same
// local dates written out independently.
const now = new Date(2026, 7, 2, 15, 30, 0).getTime();

describe("computeStatsWindow", () => {
  it("today spans the local calendar day from midnight to now", () => {
    const midnight = new Date(2026, 7, 2, 0, 0, 0, 0).getTime();
    expect(computeStatsWindow("today", null, null, now)).toEqual({
      from: midnight,
      to: now,
    });
  });

  it("last24h is a rolling window ending at now", () => {
    expect(computeStatsWindow("last24h", null, null, now)).toEqual({
      from: now - 24 * 3600 * 1000,
      to: now,
    });
  });

  it("last7d is a rolling window ending at now", () => {
    expect(computeStatsWindow("last7d", null, null, now)).toEqual({
      from: now - 7 * 24 * 3600 * 1000,
      to: now,
    });
  });

  it("custom spans start-day midnight to end-day 24:00 (next day midnight)", () => {
    expect(computeStatsWindow("custom", "2026-08-01", "2026-08-03", now)).toEqual({
      from: new Date(2026, 7, 1).getTime(),
      to: new Date(2026, 7, 4).getTime(),
    });
  });

  it("custom accepts the current day as the end date without clipping the right bound", () => {
    expect(computeStatsWindow("custom", "2026-08-02", "2026-08-02", now)).toEqual({
      from: new Date(2026, 7, 2).getTime(),
      to: new Date(2026, 7, 3).getTime(),
    });
  });

  it("custom end date lands on the local next-day midnight across DST transitions", () => {
    const from = new Date(2026, 2, 7, 0, 0, 0, 0).getTime();
    const to = new Date(2026, 2, 9, 0, 0, 0, 0).getTime();
    expect(computeStatsWindow("custom", "2026-03-07", "2026-03-08", now)).toEqual({
      from,
      to,
    });
  });

  it("throws when a custom window is missing either date", () => {
    expect(() => computeStatsWindow("custom", null, "2026-08-03", now)).toThrow();
    expect(() => computeStatsWindow("custom", "2026-08-01", "", now)).toThrow();
  });
});
