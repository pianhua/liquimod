import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import CustomSelect from "./CustomSelect.svelte";

const options = [
  { value: "custom", label: "自定义拖拽" },
  { value: "name", label: "按名称 A-Z" },
  { value: "enabled", label: "启用状态置顶" },
];

describe("CustomSelect", () => {
  it("鼠标离开菜单后清除非选中项的悬停高亮", async () => {
    render(CustomSelect, { props: { value: "custom", options } });

    await fireEvent.click(screen.getByRole("combobox"));
    const listbox = screen.getByRole("listbox");
    const option = screen.getByRole("option", { name: "启用状态置顶" });

    await fireEvent.mouseEnter(option);
    expect(option.className.split(/\s+/)).toContain("bg-[var(--item-hover)]");

    await fireEvent.mouseLeave(listbox);
    expect(option.className.split(/\s+/)).not.toContain("bg-[var(--item-hover)]");
  });
});
