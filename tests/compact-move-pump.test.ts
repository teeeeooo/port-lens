import { describe, expect, it, vi } from "vitest";
import { createCompactMovePump } from "../src/compactMovePump";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>(yes => { resolve = yes; });
  return { promise, resolve };
}

describe("latest-only compact move pump", () => {
  it("coalesces a burst into one latest sample without losing a pending hide", async () => {
    const first = deferred();
    const move = vi.fn().mockImplementationOnce(() => first.promise).mockResolvedValue(undefined);
    const pump = createCompactMovePump(move);
    pump.request();
    for (let i = 0; i < 100; i++) pump.request(i === 50);
    expect(move).toHaveBeenCalledTimes(1);
    const done = pump.finish(true);
    first.resolve();
    await done;
    expect(move).toHaveBeenCalledTimes(2);
    expect(move).toHaveBeenLastCalledWith(true);
  });

  it("cancel overrides a queued final release sample", async () => {
    const first = deferred();
    const move = vi.fn(() => first.promise);
    const pump = createCompactMovePump(move);
    pump.request();
    const released = pump.finish(true);
    const cancelled = pump.finish(false);
    first.resolve();
    await Promise.all([released, cancelled]);
    expect(move).toHaveBeenCalledTimes(1);
    pump.request();
    expect(move).toHaveBeenCalledTimes(1);
  });

  it("returns a move failure once drained, without unhandled rejection or retry storm", async () => {
    const failure = new Error("move failed");
    const move = vi.fn().mockRejectedValue(failure);
    const pump = createCompactMovePump(move);
    pump.request(true);
    expect(await pump.finish(true)).toBe(failure);
    expect(move).toHaveBeenCalledTimes(1);
  });

  it("finish waits for a sample arriving between a drain and its finalizer", async () => {
    const first = deferred();
    const second = deferred();
    const move = vi.fn().mockImplementationOnce(() => first.promise).mockImplementationOnce(() => second.promise);
    const pump = createCompactMovePump(move);
    pump.request();
    first.resolve();
    await Promise.resolve();
    pump.request();
    let finished = false;
    const done = pump.finish(true).then(() => { finished = true; });
    for (let i = 0; i < 5; i++) await Promise.resolve();
    expect(move).toHaveBeenCalledTimes(2);
    expect(finished).toBe(false);
    second.resolve();
    await done;
    expect(finished).toBe(true);
  });
});
