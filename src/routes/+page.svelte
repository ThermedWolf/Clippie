<script lang="ts">
  import { onMount } from "svelte";

  import OverlayBar from "$lib/components/overlay-bar.svelte";
  import { getRecentClips } from "$lib/tauri/commands";
  import { onClipboardUpdated } from "$lib/tauri/clipboard-events";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  let clipCount = $state(0);

  onMount(() => {
    getRecentClips()
      .then((clips) => (clipCount = clips.length))
      .catch((error) => console.error("failed to load recent clips", error));

    // The Rust core only emits this event from part 2 on; until then the badge
    // simply stays at 0 unless a test payload arrives.
    let unlisten: UnlistenFn | undefined;
    onClipboardUpdated((clip) => {
      clipCount += 1;
      console.debug("clipboard updated", clip.id, clip.kind);
    })
      .then((stop) => (unlisten = stop))
      .catch((error) => console.error("failed to subscribe to clipboard events", error));

    return () => unlisten?.();
  });
</script>

<OverlayBar {clipCount} />
