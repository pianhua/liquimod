<script lang="ts">
  let {
    checked,
    onchange,
    ariaLabel = "启用",
  }: {
    checked: boolean;
    onchange: (next: boolean) => void;
    ariaLabel?: string;
  } = $props();
</script>

<button
  role="switch"
  aria-label={ariaLabel}
  aria-checked={checked}
  class="radius-pill relative w-11 h-7 transition-colors duration-200 shrink-0"
  style:background={checked ? "var(--accent)" : "var(--glass-stroke)"}
  onclick={() => onchange(!checked)}
>
  <span
    class="absolute top-0.5 left-0.5 w-6 h-6 rounded-full bg-white shadow transition-transform duration-200"
    style:transform={checked ? "translateX(16px)" : "translateX(0)"}
  ></span>
</button>