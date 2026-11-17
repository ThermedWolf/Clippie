import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { CLIPBOARD_UPDATED_EVENT } from "$types/events";
import type { ClipSummary } from "$types/clip";

/**
 * Subscribes to clipboard captures emitted by the Rust core.
 * Resolves to an unlisten function that must be called on teardown.
 */
export function onClipboardUpdated(
  handler: (clip: ClipSummary) => void,
): Promise<UnlistenFn> {
  return listen<ClipSummary>(CLIPBOARD_UPDATED_EVENT, (event) =>
    handler(event.payload),
  );
}
