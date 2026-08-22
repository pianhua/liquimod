import { act, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import Toast from "./Toast.svelte";
import { toast, toasts } from "$lib/toast.svelte";

describe("Toast 组件", () => {
  afterEach(() => {
    toasts.length = 0;
  });

  it("渲染 store 中的消息", async () => {
    toasts.length = 0;
    render(Toast);
    await act(() => {
      toast("检测到仓库变动：+1 / -0");
    });
    expect(screen.getByRole("status")).toHaveTextContent("检测到仓库变动：+1 / -0");
  });

  it("相同消息在短时间内只保留一个 Toast", async () => {
    toasts.length = 0;
    render(Toast);
    await act(() => {
      toast("重复错误");
      toast("重复错误");
    });
    expect(screen.getAllByRole("status")).toHaveLength(1);
  });
});
