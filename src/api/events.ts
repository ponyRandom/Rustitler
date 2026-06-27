import { listen } from "@tauri-apps/api/event";
import type { BatchEvent } from "../types/ipc";

export const BATCH_EVENT_NAME = "batch-event";

export type StopListening = () => void;

export const subscribeBatchEvents = async (
  onEvent: (event: BatchEvent) => void,
): Promise<StopListening> => {
  const unlisten = await listen<BatchEvent>(BATCH_EVENT_NAME, (event) => {
    onEvent(event.payload);
  });

  return unlisten;
};
