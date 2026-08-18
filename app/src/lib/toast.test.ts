import { beforeEach, describe, expect, it, vi } from "vitest";
import { toast, toasts } from "./toast.svelte";

describe("toast store", () => {
  beforeEach(() => {
    toasts.length = 0;
    vi.useFakeTimers();
    return () => vi.useRealTimers();
  });

  it("push 后到时自动移除", () => {
    toast("hello");
    expect(toasts).toHaveLength(1);
    vi.advanceTimersByTime(4000);
    expect(toasts).toHaveLength(0);
  });

  it("多条独立计时", () => {
    toast("a", 1000);
    toast("b", 5000);
    vi.advanceTimersByTime(1000);
    expect(toasts.map((t) => t.message)).toEqual(["b"]);
  });
});
