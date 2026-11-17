<script lang="ts">
  interface Props {
    /** Rendered size in px (width and height). */
    size?: number;
  }

  let { size = 18 }: Props = $props();
</script>

<!-- Decorative brand glyph; meaning is carried by the adjacent "Clippie" text. -->
<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48" />
</svg>
