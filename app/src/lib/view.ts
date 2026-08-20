import type { ModDto } from "$lib/api";

export type View =
  | { kind: "home" } // 角色网格（虚拟「角色」大类）
  | { kind: "type"; id: number; name: string } // 实体分类
  | { kind: "character"; name: string; display: string; categoryId?: number | null; categoryName?: string }; // 某角色详情

export type ModSort = "custom" | "recent" | "name" | "enabled" | "size";
export type EnabledFilter = "all" | "on" | "off";

export function viewKey(v: View): string {
  switch (v.kind) {
    case "home":
      return "home";
    case "type":
      return `type:${v.id}`;
    case "character":
      return `char:${v.name}:${v.categoryId ?? "root"}`;
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
  return arr.sort((a, b) => {
    // 💖 喜爱/置顶优先
    const favA = a.is_favorite ? 1 : 0;
    const favB = b.is_favorite ? 1 : 0;
    if (favA !== favB) return favB - favA;

    switch (sort) {
      case "custom":
        return (a.sort_order ?? 0) - (b.sort_order ?? 0);
      case "recent":
        return b.installed_at - a.installed_at;
      case "name":
        return a.name.localeCompare(b.name, "zh-Hans-CN");
      case "enabled":
        return Number(b.enabled) - Number(a.enabled) || a.name.localeCompare(b.name, "zh-Hans-CN");
      case "size":
        return b.size_bytes - a.size_bytes;
      default:
        return 0;
    }
  });
}
