/**
 * 全局层级 Escape 调度器
 * 保证无论组件挂载顺序如何，永远是处于最顶层的弹窗/画廊/菜单/状态优先消费 Esc，层级递退。
 */

type EscHandler = () => boolean; // 返回 true 表示已消费该 Esc，阻止后续向下冒泡

const escStack: EscHandler[] = [];

/**
 * 注册一个 Esc 拦截器（后注册的优先级更高，即 LIFO 栈顶优先）
 * @returns 销毁注销函数
 */
export function pushEscHandler(handler: EscHandler): () => void {
  escStack.push(handler);
  return () => {
    const idx = escStack.lastIndexOf(handler);
    if (idx >= 0) {
      escStack.splice(idx, 1);
    }
  };
}

/**
 * 在全局 keydown 事件中分发 Esc 按键
 * @returns 是否有层级已消费 Esc
 */
export function dispatchEscape(): boolean {
  for (let i = escStack.length - 1; i >= 0; i--) {
    const consumed = escStack[i]();
    if (consumed) {
      return true;
    }
  }
  return false;
}
