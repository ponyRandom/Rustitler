import { describe, expect, it, vi } from "vitest";
import { batchEvent } from "../test/fixtures";

const listenMock = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

describe("batch event subscription", () => {
  it("subscribes to the backend event name and forwards payloads", async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);
    const { subscribeBatchEvents } = await import("./events");
    const received = vi.fn();

    const stop = await subscribeBatchEvents(received);
    const handler = listenMock.mock.calls[0][1];
    handler({
      event: "batch-event",
      id: 1,
      payload: batchEvent({
        type: "BatchCompleted",
        batchId: "batch-1",
        summary: {
          total: 1,
          outputCreated: 1,
          pending: 0,
          skipped: 0,
          failed: 0,
          cancelled: 0,
        },
      }),
    });
    stop();

    expect(listenMock).toHaveBeenCalledWith("batch-event", expect.any(Function));
    expect(received).toHaveBeenCalledWith(
      expect.objectContaining({ type: "BatchCompleted", batchId: "batch-1" }),
    );
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
