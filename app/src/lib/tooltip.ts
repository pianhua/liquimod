/**
 * 全局 iOS 质感 Tooltip 状态机与智能定位引擎
 */

export interface TooltipInfo {
  text: string;
  shortcut?: string;
  x: number;
  y: number;
  placement: "top" | "bottom";
}

let activeTarget: HTMLElement | null = null;
let showTimer: number | null = null;
let subscribers = new Set<(info: TooltipInfo | null) => void>();
let currentInfo: TooltipInfo | null = null;

export function subscribeTooltip(fn: (info: TooltipInfo | null) => void) {
  subscribers.add(fn);
  fn(currentInfo);
  return () => {
    subscribers.delete(fn);
  };
}

function notify(info: TooltipInfo | null) {
  currentInfo = info;
  for (const s of subscribers) s(info);
}

/**
 * 智能解析 Tooltip 文本中的快捷键括号，例如 "在文件夹中打开 (Ctrl+O)"
 */
export function parseTooltipContent(raw: string): { main: string; shortcut?: string } {
  const trimmed = raw.trim();
  const match = trimmed.match(/^(.*?)(?:\s*[（(]([^）)]+)[）)])$/);
  if (match && match[2] && match[2].length <= 15) {
    const main = match[1].trim();
    if (main) {
      return { main, shortcut: match[2].trim() };
    }
  }
  return { main: trimmed };
}

function findTooltipTarget(el: HTMLElement | null): { target: HTMLElement; text: string } | null {
  let curr = el;
  while (curr && curr !== document.body && curr !== document.documentElement) {
    // 优先读取 data-liquimod-tip，其次读取原生 title 或 data-tooltip
    const tip = curr.getAttribute("data-liquimod-tip") || curr.getAttribute("data-tooltip");
    if (tip && tip.trim()) {
      return { target: curr, text: tip.trim() };
    }
    const nativeTitle = curr.getAttribute("title");
    if (nativeTitle && nativeTitle.trim()) {
      const text = nativeTitle.trim();
      // 将原生 title 移入 data-liquimod-tip，并清除 title 属性以屏蔽系统黑框
      curr.setAttribute("data-liquimod-tip", text);
      curr.removeAttribute("title");
      return { target: curr, text };
    }
    curr = curr.parentElement;
  }
  return null;
}

function calculatePosition(rect: DOMRect): { x: number; y: number; placement: "top" | "bottom" } {
  const gap = 8;
  const centerX = rect.left + rect.width / 2;
  // 视口安全边距
  const padding = 12;
  const clampedX = Math.max(padding, Math.min(window.innerWidth - padding, centerX));

  // 默认放置在下方，若下方空间不足（预估高度 40px）则翻转至上方
  const hasBottomSpace = rect.bottom + 48 <= window.innerHeight;
  const placement: "top" | "bottom" = hasBottomSpace ? "bottom" : "top";
  const y = placement === "bottom" ? rect.bottom + gap : rect.top - gap;

  return { x: clampedX, y, placement };
}

function hideTooltip() {
  if (showTimer !== null) {
    clearTimeout(showTimer);
    showTimer = null;
  }
  activeTarget = null;
  if (currentInfo !== null) {
    notify(null);
  }
}

/**
 * 初始化全局 Tooltip 事件监听器
 */
export function initGlobalTooltip(): () => void {
  if (typeof window === "undefined") return () => {};

  function handlePointerOver(e: PointerEvent) {
    const found = findTooltipTarget(e.target as HTMLElement);
    if (!found) {
      hideTooltip();
      return;
    }

    if (found.target === activeTarget) return;

    hideTooltip();
    activeTarget = found.target;

    // 200ms iOS 响应延迟，避免掠过时闪烁
    showTimer = window.setTimeout(() => {
      if (!activeTarget || !document.body.contains(activeTarget)) {
        hideTooltip();
        return;
      }
      const rect = activeTarget.getBoundingClientRect();
      const pos = calculatePosition(rect);
      const parsed = parseTooltipContent(found.text);

      notify({
        text: parsed.main,
        shortcut: parsed.shortcut,
        x: pos.x,
        y: pos.y,
        placement: pos.placement,
      });
    }, 200);
  }

  function handlePointerOut(e: PointerEvent) {
    const related = e.relatedTarget as HTMLElement | null;
    if (activeTarget && activeTarget.contains(related)) {
      return;
    }
    hideTooltip();
  }

  function handlePointerDown() {
    hideTooltip();
  }

  function handleScroll() {
    hideTooltip();
  }

  window.addEventListener("pointerover", handlePointerOver, { capture: true, passive: true });
  window.addEventListener("pointerout", handlePointerOut, { capture: true, passive: true });
  window.addEventListener("pointerdown", handlePointerDown, { capture: true, passive: true });
  window.addEventListener("scroll", handleScroll, { capture: true, passive: true });

  return () => {
    hideTooltip();
    window.removeEventListener("pointerover", handlePointerOver, { capture: true });
    window.removeEventListener("pointerout", handlePointerOut, { capture: true });
    window.removeEventListener("pointerdown", handlePointerDown, { capture: true });
    window.removeEventListener("scroll", handleScroll, { capture: true });
  };
}
