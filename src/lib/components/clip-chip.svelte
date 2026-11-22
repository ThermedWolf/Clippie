<script lang="ts">
  import type { ClipSummary } from "$types/clip";

  interface Props {
    clip: ClipSummary;
  }

  let { clip }: Props = $props();
</script>

<!-- Non-interactive in the spike: chips become paste actions in part 2, so no fake button affordance yet. -->
<span class="chip" data-kind={clip.kind} title={clip.preview}>{clip.preview}</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    min-width: 0;
    max-width: 112px;
    height: 28px;
    padding: 0 10px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 12px;
    line-height: 1;
    color: var(--color-ink-muted);
    background: var(--color-chip);
    border: 1px solid var(--color-hairline);
    border-radius: 9999px;
  }
</style>
