<script lang="ts">
  import { onMount } from "svelte";

  import OverlayBar from "$lib/components/overlay-bar.svelte";
  import { getRecentClips } from "$lib/tauri/commands";
  import { onClipboardUpdated } from "$lib/tauri/clipboard-events";
  import type { ClipSummary } from "$types/clip";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // Newest first. Mirrors MAX_HISTORY_ROWS in src-tauri/src/store.rs: the
  // store prunes at 500, so the live list is clamped to the same cap to stay
  // in sync over a long session.
  const MAX_LIVE_CLIPS = 500;

  // The store caps its history, so one page is the whole list and the bar's
  // badge count is the real total.
  let clips = $state<ClipSummary[]>([]);

  onMount(() => {
    getRecentClips()
      .then((recent) => (clips = recent))
      .catch((error) => console.error("failed to load recent clips", error));

    // The core only emits for genuinely new clips (repeats of the newest clip
    // are deduped away), so prepending keeps this in sync with the store.
    let unlisten: UnlistenFn | undefined;
    onClipboardUpdated((clip) => {
      clips = [clip, ...clips].slice(0, MAX_LIVE_CLIPS);
    })
      .then((stop) => (unlisten = stop))
      .catch((error) => console.error("failed to subscribe to clipboard events", error));

    return () => unlisten?.();
  });
</script>

<OverlayBar {clips} />
