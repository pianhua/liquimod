export interface ToastItem {
  id: number;
  message: string;
}

let nextId = 1;
export const toasts = $state<ToastItem[]>([]);
const timers = new Map<number, ReturnType<typeof setTimeout>>();

export function toast(message: string, durationMs = 4000): void {
  const existing = toasts.find((item) => item.message === message);
  if (existing) {
    const oldTimer = timers.get(existing.id);
    if (oldTimer) clearTimeout(oldTimer);
    const timer = setTimeout(() => {
      const i = toasts.findIndex((t) => t.id === existing.id);
      if (i >= 0) toasts.splice(i, 1);
      timers.delete(existing.id);
    }, durationMs);
    timers.set(existing.id, timer);
    return;
  }
  const id = nextId++;
  toasts.push({ id, message });
  const timer = setTimeout(() => {
    const i = toasts.findIndex((t) => t.id === id);
    if (i >= 0) toasts.splice(i, 1);
    timers.delete(id);
  }, durationMs);
  timers.set(id, timer);
}
