import { render, fireEvent, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ModRow from "./ModRow.svelte";
import type { ModDto } from "$lib/api";

const mod: ModDto = {
  id: 1,
  name: "Summer Skin",
  enabled: false,
  installed_at: new Date(2026, 7, 12).getTime() / 1000,
  thumb: null,
  size_bytes: 12.5 * 1024 * 1024,
  file_count: 42,
  path: "C:/mock/m",
  category_id: null,
};

function setup(overrides = {}) {
  const props = {
    mod: { ...mod },
    ontoggle: vi.fn(),
    onrename: vi.fn(async () => true),
    onuninstall: vi.fn(async () => {}),
    onopen: vi.fn(),
    ...overrides,
  };
  render(ModRow, { props });
  return props;
}

describe("ModRow", () => {
  it("显示名字与副行信息", () => {
    setup();
    screen.getByText("Summer Skin");
    screen.getByText(/12\.5 MB · 42 文件 · 8月12日/);
  });

  it("统计缺失时显示 —", () => {
    setup({ mod: { ...mod, size_bytes: -1, file_count: -1 } });
    screen.getByText(/— · — 文件/);
  });

  it("空格键切换启停", async () => {
    const p = setup();
    await fireEvent.keyDown(screen.getByRole("listitem"), { key: " " });
    expect(p.ontoggle).toHaveBeenCalledWith(true);
  });

  it("打开按钮回调", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("打开目录 Summer Skin"));
    expect(p.onopen).toHaveBeenCalled();
  });

  it("重命名：编辑→Enter 提交成功关闭", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("重命名 Summer Skin"));
    const input = screen.getByDisplayValue("Summer Skin");
    await fireEvent.input(input, { target: { value: "New Name" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(p.onrename).toHaveBeenCalledWith("New Name"));
    await waitFor(() => expect(screen.queryByDisplayValue("New Name")).toBeNull());
  });

  it("重命名失败保持编辑态", async () => {
    const p = setup({ onrename: vi.fn(async () => false) });
    await fireEvent.click(screen.getByLabelText("重命名 Summer Skin"));
    const input = screen.getByDisplayValue("Summer Skin");
    await fireEvent.input(input, { target: { value: "X" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(p.onrename).toHaveBeenCalled());
    screen.getByDisplayValue("X"); // 仍在编辑
  });

  it("Esc 取消重命名", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("重命名 Summer Skin"));
    const input = screen.getByDisplayValue("Summer Skin");
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByDisplayValue("Summer Skin")).toBeNull();
    expect(p.onrename).not.toHaveBeenCalled();
  });

  it("Esc 取消后 blur 不再提交（浏览器移除聚焦元素会派发 blur）", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("重命名 Summer Skin"));
    const input = screen.getByDisplayValue("Summer Skin");
    await fireEvent.input(input, { target: { value: "Should Not Save" } });
    await fireEvent.keyDown(input, { key: "Escape" });
    await fireEvent.blur(input); // 模拟浏览器行为
    await new Promise((r) => setTimeout(r, 0));
    expect(p.onrename).not.toHaveBeenCalled();
  });

  it("onrename 挂起期间重入被 busy 拦截", async () => {
    let resolveFn: ((v: boolean) => void) | undefined;
    const p = setup({ onrename: vi.fn(() => new Promise<boolean>((r) => (resolveFn = r))) });
    await fireEvent.click(screen.getByLabelText("重命名 Summer Skin"));
    const input = screen.getByDisplayValue("Summer Skin");
    await fireEvent.input(input, { target: { value: "X" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(p.onrename).toHaveBeenCalledTimes(1);
    resolveFn?.(true);
  });

  it("卸载需二次确认", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("卸载 Summer Skin"));
    screen.getByText(/文件将被删除/);
    await fireEvent.click(screen.getByText("取消"));
    expect(p.onuninstall).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByLabelText("卸载 Summer Skin"));
    await fireEvent.click(screen.getByText("确认卸载"));
    await waitFor(() => expect(p.onuninstall).toHaveBeenCalled());
  });
});
