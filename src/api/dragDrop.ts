import { getCurrentWindow } from "@tauri-apps/api/window";
import type { TauriDragDropEvent } from "../types/ipc";
import type { StopListening } from "./events";

export const subscribeFileDrops = async (
  onDrop: (paths: string[]) => void,
  onActiveChange?: (active: boolean) => void,
): Promise<StopListening> => {
  const unlisten = await getCurrentWindow().onDragDropEvent((event) => {
    const payload = event.payload as TauriDragDropEvent;

    if (payload.type === "enter") {
      onActiveChange?.(true);
      return;
    }

    if (payload.type === "leave") {
      onActiveChange?.(false);
      return;
    }

    if (payload.type === "drop") {
      onActiveChange?.(false);
      onDrop(payload.paths);
    }
  });

  return unlisten;
};
