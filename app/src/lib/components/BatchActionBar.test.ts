import { render, fireEvent, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import BatchActionBar from "./BatchActionBar.svelte";
import type { CategoryDto } from "$lib/api";

const categories: CategoryDto[] = [
  { id: 1, name: "立绘替换", ord: 1, kind: null, mod_count: 5 },
  { id: 2, name: "特效增强", ord: 2, kind: null, mod_count: 3 },
];

describe("BatchActionBar", () => {
  it("当选中数量为 0 时不显示", () => {
    const { container } = render(BatchActionBar, {
      props: {
        selectedCount: 0,
        categories,
        onEnableAll: vi.fn(),
        onDisableAll: vi.fn(),
        onMoveCategory: vi.fn(),
        onReassignCharacter: vi.fn(),
        onUninstallAll: vi.fn(),
        onClearSelection: vi.fn(),
      },
    });

    expect(container.querySelector('[role="toolbar"]')).toBeNull();
  });

  it("当选中数量 > 0 时显示并正确响应操作", async () => {
    const onEnableAll = vi.fn();
    const onDisableAll = vi.fn();
    const onClearSelection = vi.fn();

    render(BatchActionBar, {
      props: {
        selectedCount: 3,
        categories,
        onEnableAll,
        onDisableAll,
        onMoveCategory: vi.fn(),
        onReassignCharacter: vi.fn(),
        onUninstallAll: vi.fn(),
        onClearSelection,
      },
    });

    screen.getByText("已选中 3 项");

    const enableBtn = screen.getByText("启用");
    await fireEvent.click(enableBtn);
    expect(onEnableAll).toHaveBeenCalled();

    const disableBtn = screen.getByText("禁用");
    await fireEvent.click(disableBtn);
    expect(onDisableAll).toHaveBeenCalled();

    const clearBtn = screen.getByTitle(/取消选择/);
    await fireEvent.click(clearBtn);
    expect(onClearSelection).toHaveBeenCalled();
  });
});
