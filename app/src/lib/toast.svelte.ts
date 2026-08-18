export interface ToastItem {
  id: number;
  message: string;
}

let nextId = 1;
export const toasts = $state<ToastItem[]>([]);

export function toast(message: string, durationMs = 4000): void {
  const id = nextId++;
  toasts.push({ id, message });
  setTimeout(() => {
    const i = toasts.findIndex((t) => t.id === id);
    if (i >= 0) toasts.splice(i, 1);
  }, durationMs);
}
