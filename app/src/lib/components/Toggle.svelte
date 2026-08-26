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
  class="toggle-track relative inline-flex h-7 w-12 shrink-0 cursor-pointer rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-40"
  onclick={() => {
    if (!disabled) onchange(!checked);
  }}
>
  <span class="toggle-thumb pointer-events-none absolute h-6 w-6 rounded-full"></span>
</button>

<style>
  /* 关闭态：半透明玻璃凹槽 —— 内凹深度 + 底部折光边 */
  .toggle-track {
    background: var(--toggle-track);
    border: 0.5px solid var(--toggle-track-stroke);
    box-shadow: var(--toggle-well);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    transition:
      background 0.28s var(--ease-settle),
      box-shadow 0.28s var(--ease-settle),
      border-color 0.28s var(--ease-settle),
      transform 0.16s var(--ease-spring);
  }
  /* 开启态：accent 玻璃充填 —— 顶部内高光 + 底部内深度 + 环境光晕 */
  .toggle-track[aria-checked="true"] {
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 78%, white) 0%,
      var(--accent) 60%
    );
    border-color: transparent;
    box-shadow:
      inset 0 1px 1.5px rgba(255, 255, 255, 0.42),
      inset 0 -1.5px 3px rgba(0, 0, 0, 0.16),
      0 2px 10px color-mix(in srgb, var(--accent) 30%, transparent);
  }
  /* 滑块：垂直居中用 translate 属性（与滑动 transform 互不干扰），
     顶部 rim 高光 + 柔和落影营造「浮在凹槽上的玻璃珠」 */
  .toggle-thumb {
    top: 50%;
    left: 2px;
    translate: 0 -50%;
    transform: translateX(0);
    background: var(--toggle-thumb);
    box-shadow:
      var(--toggle-thumb-rim),
      var(--toggle-thumb-shadow);
    transition:
      transform 0.32s var(--ease-spring),
      width 0.32s var(--ease-spring);
  }
  .toggle-track[aria-checked="true"] .toggle-thumb {
    transform: translateX(20px);
  }
  /* 按压时滑块沿运动方向拉伸（iOS 液态形变） */
  .toggle-track:active:not(:disabled) .toggle-thumb {
    width: 28px;
  }
  .toggle-track[aria-checked="true"]:active:not(:disabled) .toggle-thumb {
    transform: translateX(16px);
  }
</style>
