<script lang="ts">
  let {
    checked,
    disabled = false,
    onchange,
    ariaLabel = "启用",
  }: {
    checked: boolean;
    disabled?: boolean;
    onchange: (next: boolean) => void;
    ariaLabel?: string;
  } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-label={ariaLabel}
  aria-checked={checked}
  {disabled}
  class="relative inline-flex h-7 w-12 shrink-0 cursor-pointer items-center rounded-full transition-colors duration-250 ease-out focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-40"
  style:background={checked ? "var(--accent)" : "var(--island-badge)"}
  style:border={checked ? "0.5px solid rgba(0, 0, 0, 0.08)" : "0.5px solid var(--glass-stroke)"}
  style:box-shadow={checked ? "inset 0 1px 2px rgba(0, 0, 0, 0.15)" : "none"}
  onclick={() => {
    if (!disabled) onchange(!checked);
  }}
>
  <span
    class="toggle-thumb pointer-events-none absolute left-0.5 top-0.5 h-6 w-6 rounded-full bg-white transition-all duration-280 ease-[cubic-bezier(0.34,1.56,0.64,1)]"
    style:transform={checked ? "translateX(20px)" : "translateX(0)"}
    style:box-shadow="0 3px 8px rgba(0,0,0,0.18), 0 1px 2px rgba(0,0,0,0.12), 0 0 1px rgba(0,0,0,0.2)"
  ></span>
</button>

<style>
  button:active:not(:disabled) .toggle-thumb {
    width: 28px;
  }
  button[aria-checked="true"]:active:not(:disabled) .toggle-thumb {
    transform: translateX(16px);
  }
</style>