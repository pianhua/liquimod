import { describe, expect, it } from "vitest";
import { filterMods, mergeVisibleOrder, sortMods, viewKey, type View } from "./view";
import type { ModDto } from "./api";

function mod(id: number, name: string, enabled: boolean, installed_at: number): ModDto {
  return { id, name, enabled, installed_at, thumb: null, size_bytes: 0, file_count: 0, path: "", category_id: null, note: null, cover_image: null };
}

describe("viewKey", () => {
  it("每种视图唯一", () => {
    const keys = [
      viewKey({ kind: "home" }),
      viewKey({ kind: "type", id: 1, name: "光锥" }),
      viewKey({ kind: "type", id: 2, name: "NPC" }),
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

  it("按启用态过滤", () => {
    const mods = [mod(1, "A", true, 0), mod(2, "B", false, 0)];
    expect(filterMods(mods, "", "on").map((m) => m.id)).toEqual([1]);
    expect(filterMods(mods, "", "off").map((m) => m.id)).toEqual([2]);
    expect(filterMods(mods, "", "all")).toHaveLength(2);
  });

  it("名称与启用态组合", () => {
    const mods = [mod(1, "Summer", true, 0), mod(2, "Summer", false, 0), mod(3, "Winter", true, 0)];
    expect(filterMods(mods, "summer", "on").map((m) => m.id)).toEqual([1]);
  });

  it("默认不过滤启用态", () => {
    const mods = [mod(1, "A", true, 0), mod(2, "B", false, 0)];
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

describe("mergeVisibleOrder", () => {
  it("只重排筛选可见项并保留隐藏项槽位", () => {
    expect(mergeVisibleOrder([1, 2, 3, 4, 5], [5, 3, 1])).toEqual([5, 2, 3, 4, 1]);
  });
});
