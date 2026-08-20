/**
 * 全局 iOS / Liquid Glass 质感 Tooltip 状态机与智能定位引擎
 */

export interface TooltipInfo {
  text: string;
  shortcut?: string;
  x: number;
  y: number;
  placement: "top" | "bottom";
  align: "left" | "center" | "right";
}

let activeTarget: HTMLElement | null = null;
let showTimer: number | null = null;
let subscribers = new Set<(info: TooltipInfo | null) => void>();
let currentInfo: TooltipInfo | null = null;

// 使用 WeakMap 暂存元素原始 title，在移出时自动还原，保证 Svelte 响应式 title 变更不丢失
const originalTitles = new WeakMap<HTMLElement, string>();

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
 * 严格判断一段字符串是否为真正的键盘快捷键
 */
export function isKeyShortcut(str: string): boolean {
  const t = str.trim();
  if (!t || t.length > 20) return false;

  // 1. 包含 Ctrl / Cmd / Alt / Shift / Win / Meta / Option 组合键
  if (/^(?:ctrl|cmd|alt|shift|win|meta|opt|option)(?:\s*\+\s*(?:[a-z0-9]|f[1-9][0-2]?|space|enter|esc|tab|del|delete|backspace|arrow[a-z]+|\/|\?|\.|\,|\-|\=))+$/i.test(t)) {
    return true;
  }

  // 2. 独立功能键 (F1-F12, Esc, Space, Enter, Tab, Del, etc.)
  if (/^(?:f[1-9]|f1[0-2]|esc|escape|space|enter|return|tab|del|delete|backspace|insert|home|end|pageup|pagedown)$/i.test(t)) {
    return true;
  }

  // 3. 数字小键盘键 (Num *, Num /, Num 1, etc.)
  if (/^num\s*(?:[\d\.\*\+\-\/]|lock)$/i.test(t)) {
    return true;
  }

  // 4. 单字符按键与符号 (A-Z, 0-9, -, +, =, /, ?, ., etc.)
  if (/^[A-Za-z0-9\/\?\+\-\=\.\,\;]$/.test(t)) {
    return true;
  }

  return false;
}

/**
 * 智能解析 Tooltip 文本中的快捷键括号，例如 "搜索角色或 Mod (Ctrl+K)"
 */
export function parseTooltipContent(raw: string): { main: string; shortcut?: string } {
  const trimmed = raw.trim();
  const match = trimmed.match(/^(.*?)(?:\s*[（(]([^）)]+)[）)])$/);
  if (match && match[2]) {
    const candidate = match[2].trim();
    if (isKeyShortcut(candidate)) {
      const main = match[1].trim();
      if (main) {
        return { main, shortcut: candidate };
      }
    }
  }
  return { main: trimmed };
}

function findTooltipTarget(el: HTMLElement | null): { target: HTMLElement; text: string } | null {
  let curr = el;
  while (curr && curr !== document.body && curr !== document.documentElement) {
    // 动态检查当前最新的 title 或 data-liquimod-tip / data-tooltip
    const nativeTitle = curr.getAttribute("title");
    if (nativeTitle && nativeTitle.trim()) {
      const text = nativeTitle.trim();
      originalTitles.set(curr, text);
      curr.setAttribute("data-liquimod-tip", text);
      // 临时屏蔽原生 title 避免浏览器黑框
      curr.removeAttribute("title");
      return { target: curr, text };
    }

    const tip = curr.getAttribute("data-liquimod-tip") || curr.getAttribute("data-tooltip");
    if (tip && tip.trim()) {
      return { target: curr, text: tip.trim() };
    }

    curr = curr.parentElement;
  }
  return null;
}

function calculatePosition(rect: DOMRect): { x: number; y: number; placement: "top" | "bottom"; align: "left" | "center" | "right" } {
  const gap = 8;
  const padding = 16;
  const viewportWidth = window.innerWidth;
  const centerX = rect.left + rect.width / 2;

  // 智能边缘对齐：靠近右边缘靠右对齐，靠近左边缘靠左对齐，中间居中
  let align: "left" | "center" | "right" = "center";
  let x = centerX;

  if (rect.right > viewportWidth - 120) {
    align = "right";
    x = Math.min(viewportWidth - padding, rect.right);
  } else if (rect.left < 120) {
    align = "left";
    x = Math.max(padding, rect.left);
  } else {
    align = "center";
    x = Math.max(padding, Math.min(viewportWidth - padding, centerX));
  }

  // 默认放置在下方，若下方空间不足（预估高度 48px）则翻转至上方
  const hasBottomSpace = rect.bottom + 52 <= window.innerHeight;
  const placement: "top" | "bottom" = hasBottomSpace ? "bottom" : "top";
  const y = placement === "bottom" ? rect.bottom + gap : rect.top - gap;

  return { x, y, placement, align };
}

function hideTooltip() {
  if (showTimer !== null) {
    clearTimeout(showTimer);
    showTimer = null;
  }
  if (activeTarget) {
    // 鼠标离开时，若有暂存的 title 则恢复回原生属性，确保响应式更新顺畅
    const saved = originalTitles.get(activeTarget);
    if (saved && !activeTarget.hasAttribute("title")) {
      activeTarget.setAttribute("title", saved);
    }
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

    // 160ms 触感延迟，避免高速掠过时闪烁
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
        align: pos.align,
      });
    }, 160);
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
