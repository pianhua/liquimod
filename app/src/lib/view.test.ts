import { describe, expect, it } from "vitest";
import { filterMods, sortMods, viewKey, type View } from "./view";
import type { ModDto } from "./api";

function mod(id: number, name: string, enabled: boolean, installed_at: number): ModDto {
  return { id, name, enabled, installed_at, thumb: null, size_bytes: 0, file_count: 0, path: "", category_id: null };
}

describe("viewKey", () => {
  it("每种视图唯一", () => {
    const keys = [
      viewKey({ kind: "home" }),
      viewKey({ kind: "all" }),
      viewKey({ kind: "uncat" }),
      viewKey({ kind: "category", id: 1, name: "A" }),
      viewKey({ kind: "category", id: 2, name: "A" }),
      viewKey({ kind: "character", name: "Firefly", display: "流萤" }),
    ];
    expect(new Set(keys).size).toBe(keys.length);
  });
});

describe("filterMods", () => {
  it("按名称不区分大小写过滤", () => {
    const mods = [mod(1, "Summer Skin", false, 0), mod(2, "战斗特效", false, 0)];
    expect(filterMods(mods, "summer").map((m) => m.id)).toEqual([1]);
    expect(filterMods(mods, "战斗").map((m) => m.id)).toEqual([2]);
    expect(filterMods(mods, "")).toHaveLength(2);
  });
});

describe("sortMods", () => {
  const mods = [
    mod(1, "B", false, 100),
    mod(2, "A", true, 50),
    mod(3, "C", true, 200),
  ];
  it("recent 按安装时间倒序", () => {
    expect(sortMods(mods, "recent").map((m) => m.id)).toEqual([3, 1, 2]);
  });
  it("name 按名称", () => {
    expect(sortMods(mods, "name").map((m) => m.id)).toEqual([2, 1, 3]);
  });
  it("enabled 启用优先再按名称", () => {
    expect(sortMods(mods, "enabled").map((m) => m.id)).toEqual([2, 3, 1]);
  });
  it("不改变原数组", () => {
    sortMods(mods, "recent");
    expect(mods.map((m) => m.id)).toEqual([1, 2, 3]);
  });
});
