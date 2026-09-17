/** Latest-only move requests: one invoke in flight, at most one pending sample.
 * Coordinates remain owned by Rust; never replay historical pointer positions.
 */
export function createCompactMovePump(move: (hideHover: boolean) => Promise<void>) {
  let pending = false;
  let hidePending = false;
  let accepting = true;
  let worker: Promise<void> | null = null;
  let failure: unknown;

  const drain = () => {
    if (worker) return worker;
    worker = (async () => {
      while (pending) {
        const hide = hidePending;
        pending = false;
        hidePending = false;
        try {
          await move(hide);
        } catch (error) {
          failure = error;
          accepting = false;
          pending = false;
          break;
        }
      }
    })().finally(() => {
      worker = null;
      // A request can arrive in the microtask between drain completion and
      // this finalizer. Do not leave that latest sample stranded.
      if (pending) void drain();
    });
    return worker;
  };

  return {
    request(hideHover = false) {
      if (!accepting) return;
      pending = true;
      hidePending ||= hideHover;
      void drain();
    },
    async finish(finalSample: boolean) {
      // Cancel may override an already queued pointerup sample (Open/new gesture).
      pending = finalSample && accepting;
      accepting = false;
      if (pending) void drain();
      // A finalizer may restart the worker for a just-arrived sample. Wait
      // until the entire pump is quiescent before releasing the polling gate.
      while (worker) await worker;
      return failure;
    },
  };
}

export type CompactMovePump = ReturnType<typeof createCompactMovePump>;
