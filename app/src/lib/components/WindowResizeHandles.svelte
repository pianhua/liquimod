<script lang="ts">
  import { isTauri } from "$lib/api";

  // @tauri-apps/api 未导出 ResizeDirection 类型，按 window.d.ts 的定义本地声明。
  type ResizeDirection =
    | "East"
    | "North"
    | "NorthEast"
    | "NorthWest"
    | "South"
    | "SouthEast"
    | "SouthWest"
    | "West";

  // 8 个透明缩放命中带，骑跨玻璃壳体边缘（样式见 app.css .resize-handle）。
  // 外层 32px 透明外延内的拖拽由此转发给系统原生缩放，最大化时由 CSS 隐藏。
  const handles: [string, ResizeDirection][] = [
    ["resize-n", "North"],
    ["resize-s", "South"],
    ["resize-w", "West"],
    ["resize-e", "East"],
    ["resize-nw", "NorthWest"],
    ["resize-ne", "NorthEast"],
    ["resize-sw", "SouthWest"],
    ["resize-se", "SouthEast"],
  ];

  async function startResize(direction: ResizeDirection, event: MouseEvent) {
    if (event.button !== 0 || !isTauri()) return;
    event.preventDefault();
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().startResizeDragging(direction);
  }
</script>

{#each handles as [cls, direction] (cls)}
  <div
    class="resize-handle {cls}"
    aria-hidden="true"
    onmousedown={(event) => startResize(direction, event)}
  ></div>
{/each}
