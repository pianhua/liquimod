import type { ModDto } from "$lib/api";

export type View =
  | { kind: "home" } // 角色网格（虚拟「角色」大类）
  | { kind: "type"; id: number; name: string } // 光锥/立绘/场景/NPC/其他 实体分类
  | { kind: "character"; name: string; display: string }; // 某角色详情

export type ModSort = "recent" | "name" | "enabled";
export type EnabledFilter = "all" | "on" | "off";

export function viewKey(v: View): string {
  switch (v.kind) {
    case "home":
      return "home";
    case "type":
      return `type:${v.id}`;
    case "character":
      return `char:${v.name}`;
  }
}

export function filterMods(
  mods: ModDto[],
  query: string,
  enabledFilter: EnabledFilter = "all",
): ModDto[] {
  const q = query.trim().toLowerCase();
  return mods.filter((m) => {
    if (enabledFilter === "on" && !m.enabled) return false;
    if (enabledFilter === "off" && m.enabled) return false;
    if (!q) return true;
    return m.name.toLowerCase().includes(q);
  });
}

export function sortMods(mods: ModDto[], sort: ModSort): ModDto[] {
  const arr = [...mods];
  switch (sort) {
    case "recent":
      return arr.sort((a, b) => b.installed_at - a.installed_at);
    case "name":
      return arr.sort((a, b) => a.name.localeCompare(b.name, "zh-Hans-CN"));
    case "enabled":
      return arr.sort(
        (a, b) => Number(b.enabled) - Number(a.enabled) || a.name.localeCompare(b.name, "zh-Hans-CN"),
      );
  }
}
