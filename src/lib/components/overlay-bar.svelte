<script lang="ts">
  import ClipChip from "./clip-chip.svelte";
  import ClippieMark from "./clippie-mark.svelte";
  import type { ClipSummary } from "$types/clip";

  interface Props {
    clipCount: number;
  }

  let { clipCount }: Props = $props();

  // Clicking the brand pill pins the bar open (keyboard users cannot hover).
  // It stays open until clicked again or focus/hover leaves the bar.
  let pinned = $state(false);

  // Placeholder history for the spike; the real clips arrive via IPC in part 2.
  const placeholderClips: ClipSummary[] = [
    { id: "demo-1", kind: "text", preview: "Copied text lands here…", createdAt: 0 },
    { id: "demo-2", kind: "image", preview: "screenshot-2026-11-16.png", createdAt: 0 },
    { id: "demo-3", kind: "files", preview: "3 files — ~/Projects", createdAt: 0 },
  ];
</script>

<section class="dock" aria-label="Clippie clipboard bar">
  <div class="bar" class:pinned>
    <button
      type="button"
      class="pill"
      onclick={() => (pinned = !pinned)}
      aria-expanded={pinned}
      title="Pin the Clippie bar open"
    >
      <span class="mark"><ClippieMark /></span>
      <span class="name">Clippie</span>
      <span class="badge" title="{clipCount} clips in history">{clipCount}</span>
    </button>

    <div class="clips" aria-hidden="false">
      {#each placeholderClips as clip (clip.id)}
        <ClipChip {clip} />
      {/each}
    </div>

    <button type="button" class="settings" aria-label="Open Clippie settings" title="Settings">
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path
          d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
        />
        <circle cx="12" cy="12" r="3" />
      </svg>
    </button>
  </div>
</section>

<style>
  /* Centers the bar inside its transparent 420x56 window. */
  .dock {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    width: var(--bar-width-collapsed);
    height: var(--bar-height-collapsed);
    padding: 0 14px;
    overflow: hidden;
    background: var(--color-surface);
    border: 1px solid var(--color-hairline);
    border-radius: 9999px;
    backdrop-filter: blur(12px);
    transition:
      width var(--bar-expand-duration) var(--ease-expand),
      height var(--bar-expand-duration) var(--ease-expand),
      padding var(--bar-expand-duration) var(--ease-expand),
      border-radius var(--bar-expand-duration) var(--ease-expand),
      background-color var(--bar-expand-duration) var(--ease-expand);
  }

  /* Expanded: full bar. Triggered by hover, keyboard focus, or a pinned click. */
  .dock:hover .bar,
  .dock:focus-within .bar,
  .bar.pinned {
    width: var(--bar-width-expanded);
    height: var(--bar-height-expanded);
    padding: 0 10px;
    background: var(--color-surface-raised);
    border-radius: var(--radius-bar);
  }

  /* Content that only exists in the expanded state; fades out before it is
     hidden so collapse never clips mid-fade. */
  .clips,
  .settings {
    opacity: 0;
    visibility: hidden;
    transition:
      opacity var(--bar-expand-duration) var(--ease-expand),
      visibility 0s linear var(--bar-expand-duration);
  }

  .dock:hover .clips,
  .dock:hover .settings,
  .dock:focus-within .clips,
  .dock:focus-within .settings,
  .bar.pinned .clips,
  .bar.pinned .settings {
    opacity: 1;
    visibility: visible;
    transition-delay: 60ms, 60ms;
  }

  .pill {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    gap: 8px;
    padding: 4px;
    color: var(--color-ink);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.01em;
    background: none;
    border: none;
    border-radius: 9999px;
    cursor: pointer;
    transition: color var(--bar-expand-duration) var(--ease-expand);
  }

  .mark {
    display: inline-flex;
    color: var(--color-accent);
  }

  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    font-size: 11px;
    font-weight: 700;
    line-height: 1;
    color: var(--color-accent-ink);
    background: var(--color-accent);
    border-radius: 9999px;
  }

  .clips {
    display: flex;
    flex: 1;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
  }

  .settings {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    color: var(--color-ink-muted);
    background: none;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: color 150ms ease-out, background-color 150ms ease-out;
  }

  .settings:hover {
    color: var(--color-ink);
    background: var(--color-chip);
  }

  /* Visible keyboard focus on every interactive element. */
  .pill:focus-visible,
  .settings:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
  }

  @media (prefers-reduced-motion: reduce) {
    .bar,
    .clips,
    .settings,
    .pill,
    .settings {
      transition: none;
    }
  }
</style>
