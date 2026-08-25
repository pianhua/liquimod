import { render, fireEvent, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ModDetailPane from "./ModDetailPane.svelte";
import type { ModDto } from "$lib/api";

vi.mock("$lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("$lib/api")>();
  return {
    ...original,
    openModFolder: vi.fn(),
  };
});

const mockMod: ModDto = {
  id: 101,
  name: "Firefly Summer Skin",
  enabled: true,
  installed_at: 1700000000,
  thumb: "data:image/jpeg;base64,mockthumb",
  size_bytes: 15 * 1024 * 1024,
  file_count: 8,
  path: "E:/all in/SRMI/Mods/Firefly_Summer",
  category_id: null,
  note: null,
  cover_image: null,
};

function setup(propsOverrides = {}) {
  const props = {
    mod: mockMod,
    categories: [{ id: 1, name: "角色", ord: 0, kind: null, mod_count: 1 }],
    ontoggle: vi.fn(),
    onrename: vi.fn(async () => true),
    onuninstall: vi.fn(async () => {}),
    onopen: vi.fn(),
    onmove: vi.fn(),
    ...propsOverrides,
  };
  render(ModDetailPane, { props });
  return props;
}

describe("ModDetailPane", () => {
  it("渲染 Mod 名称、大图与元数据", () => {
    setup();
    expect(screen.getByText("Firefly Summer Skin")).toBeTruthy();
    expect(screen.getByText("已启用")).toBeTruthy();
    expect(screen.getByText("15.0 MB")).toBeTruthy();
    expect(screen.getByText("8 个文件")).toBeTruthy();
    expect(screen.getByText("归属角色")).toBeTruthy();
  });

  it("点击大开关触发 ontoggle", async () => {
    const p = setup();
    const sw = screen.getByRole("switch");
    await fireEvent.click(sw);
    expect(p.ontoggle).toHaveBeenCalledWith(false);
  });

  it("打开目录按钮只触发 onopen，不直接调用 api.openModFolder", async () => {
    const api = await import("$lib/api") as unknown as { openModFolder: ReturnType<typeof vi.fn> };
    const p = setup();
    await fireEvent.click(screen.getByText("打开目录"));
    expect(p.onopen).toHaveBeenCalledTimes(1);
    expect(api.openModFolder).not.toHaveBeenCalled();
  });

  it("空态时显示未选中提示", () => {
    setup({ mod: null });
    expect(screen.getByText("未选中 Mod")).toBeTruthy();
  });

  it("游戏运行期间锁定 LiquiMod 文件变体", () => {
    setup({
      mod: {
        ...mockMod,
        enabled: false,
        variants: [{ name: "Option A" }],
        active_variant: "Option A",
      },
      variantLocked: true,
      onvariantchange: vi.fn(async () => {}),
    });
    expect(screen.getByRole("radio", { name: "Option A" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByText(/游戏运行期间暂不可切换文件变体/)).toBeTruthy();
  });
});
