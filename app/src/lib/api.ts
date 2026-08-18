import { invoke } from "@tauri-apps/api/core";

export interface ConfigDto {
  library_root: string;
  mods_dir: string | null;
  auto_enable: boolean;
  theme: string;
  character_category_name: string;
  game_exe: string | null;
  loader_exe: string | null;
}

export interface CharacterSummary {
  internal_name: string;
  display_name: string;
  image: string | null;
  total: number;
  enabled: number;
}

export interface CategoryDto {
  id: number;
  name: string;
  ord: number;
  mod_count: number;
}

export interface ModDto {
  id: number;
  name: string;
  enabled: boolean;
  installed_at: number;
  thumb: string | null;
  size_bytes: number;
  file_count: number;
  path: string;
  category_id: number | null;
}

export interface PresetDto {
  id: number;
  name: string;
  created_at: number;
}

export interface ApplyResultDto {
  enabled: number;
  disabled: number;
}

export type InstallResult =
  | { status: "installed"; mod_id: number; name: string; character: string; warnings: string[] }
  | { status: "needs_password" };

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const mockCharacters: CharacterSummary[] = [
  ["Acheron", "acheron.png", 3, 1],
  ["Firefly", "firefly.png", 2, 2],
  ["Castorice", "castorice.png", 1, 0],
  ["Cipher", "cipher.png", 0, 0],
  ["Aglaea", "aglaea.png", 5, 3],
  ["Anaxa", "anaxa.png", 0, 0],
  ["Archer", "archer.png", 2, 1],
  ["Argenti", "argenti.png", 0, 0],
  ["Arlan", "arlan.png", 0, 0],
  ["Asta", "asta.png", 1, 1],
  ["Aventurine", "aventurine.png", 0, 0],
  ["Bailu", "bailu.png", 0, 0],
  ["Others", null, 2, 0],
].map(([name, image, total, enabled]) => ({
  internal_name: name as string,
  display_name: name === "Others" ? "其他" : (name as string),
  image: image as string | null,
  total: total as number,
  enabled: enabled as number,
}));

const mockMods: ModDto[] = [
  { id: 1, name: "Summer Skin", enabled: true, installed_at: 1755000000, thumb: null, size_bytes: 12345678, file_count: 42, path: "C:/mock/Library/mods/Firefly/Summer Skin", category_id: null },
  { id: 2, name: "Battle FX+", enabled: false, installed_at: 1755100000, thumb: null, size_bytes: 12345678, file_count: 42, path: "C:/mock/Library/mods/Firefly/Battle FX+", category_id: null },
  { id: 3, name: "HD Textures", enabled: false, installed_at: 1755200000, thumb: null, size_bytes: 12345678, file_count: 42, path: "C:/mock/Library/mods/Firefly/HD Textures", category_id: 1 },
];

const mockPresets: PresetDto[] = [
  { id: 1, name: "日常出战", created_at: 1755000000 },
  { id: 2, name: "截图模式", created_at: 1755100000 },
];

const mockPasswords: string[] = ["1234"];

const mockCategories: CategoryDto[] = [
  { id: 1, name: "武器", ord: 1, mod_count: 1 },
  { id: 2, name: "光影", ord: 2, mod_count: 0 },
];

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    switch (cmd) {
      case "get_config":
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: "auto", character_category_name: "角色", game_exe: null, loader_exe: null } as T;
      case "get_characters":
        return structuredClone(mockCharacters) as T;
      case "list_mods":
        return structuredClone(mockMods) as T;
      case "install_mod": {
        await new Promise((r) => setTimeout(r, 800));
        const p = String(args?.path ?? "");
        if (p.includes("locked") && args?.password == null)
          return { status: "needs_password" } as T;
        if (p.includes("locked") && args?.password !== "1234")
          return { status: "needs_password" } as T;
        return {
          status: "installed",
          mod_id: 99,
          name: p.split(/[\\/]/).pop()?.replace(/\.(zip|7z|rar)$/i, "") ?? "Mod",
          character: "Firefly",
          warnings: [],
        } as T;
      }
      case "uninstall_mod":
        return undefined as T;
      case "list_presets":
        return structuredClone(mockPresets) as T;
      case "save_preset": {
        const p = { id: Math.max(0, ...mockPresets.map((x) => x.id)) + 1, name: String(args?.name ?? "预设"), created_at: 1755000000 };
        const i = mockPresets.findIndex((x) => x.name === p.name);
        if (i >= 0) mockPresets[i] = { ...p, id: mockPresets[i].id };
        else mockPresets.push(p);
        return structuredClone(p) as T;
      }
      case "apply_preset":
        return { enabled: 2, disabled: 1 } as T;
      case "delete_preset": {
        const i = mockPresets.findIndex((x) => x.id === Number(args?.id));
        if (i >= 0) mockPresets.splice(i, 1);
        return undefined as T;
      }
      case "list_passwords":
        return structuredClone(mockPasswords) as T;
      case "add_password":
        mockPasswords.push(String(args?.value ?? ""));
        return undefined as T;
      case "remove_password": {
        const i = mockPasswords.indexOf(String(args?.value));
        if (i >= 0) mockPasswords.splice(i, 1);
        return undefined as T;
      }
      case "rename_mod": {
        const m = mockMods.find((x) => x.id === Number(args?.id));
        const n = String(args?.name ?? "").trim();
        if (!n) throw "名字不合法（不能为空，不能含 / \\，不能以空格或点结尾）";
        if (mockMods.some((x) => x.id !== m?.id && x.name === n)) throw `已存在同名 Mod：${n}`;
        if (m) m.name = n;
        return undefined as T;
      }
      case "set_auto_enable":
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: Boolean(args?.enabled), theme: "auto", character_category_name: "角色", game_exe: null, loader_exe: null } as T;
      case "set_theme":
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: String(args?.theme ?? "auto"), character_category_name: "角色", game_exe: null, loader_exe: null } as T;
      case "set_character_category_name": {
        const n = String(args?.name ?? "").trim();
        if (!n) throw "名称不能为空";
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: "auto", character_category_name: n, game_exe: null, loader_exe: null } as T;
      }
      case "list_categories":
        return structuredClone([...mockCategories].sort((a, b) => a.ord - b.ord)) as T;
      case "create_category": {
        const n = String(args?.name ?? "").trim();
        if (!n) throw "分类名不能为空";
        if (mockCategories.some((c) => c.name === n)) throw `分类已存在：${n}`;
        const c = { id: Math.max(0, ...mockCategories.map((x) => x.id)) + 1, name: n, ord: Math.max(0, ...mockCategories.map((x) => x.ord)) + 1, mod_count: 0 };
        mockCategories.push(c);
        return c.id as T;
      }
      case "rename_category": {
        const n = String(args?.name ?? "").trim();
        if (!n) throw "分类名不能为空";
        if (mockCategories.some((c) => c.name === n && c.id !== Number(args?.id))) throw `分类已存在：${n}`;
        const c = mockCategories.find((x) => x.id === Number(args?.id));
        if (!c) throw "分类不存在";
        c.name = n;
        return undefined as T;
      }
      case "delete_category": {
        const id = Number(args?.id);
        const i = mockCategories.findIndex((x) => x.id === id);
        if (i < 0) throw "分类不存在";
        mockCategories.splice(i, 1);
        for (const m of mockMods) if (m.category_id === id) m.category_id = null;
        return undefined as T;
      }
      case "move_category": {
        const id = Number(args?.id);
        const delta = Number(args?.delta);
        const sorted = [...mockCategories].sort((a, b) => a.ord - b.ord);
        const i = sorted.findIndex((x) => x.id === id);
        const j = i + delta;
        if (i >= 0 && j >= 0 && j < sorted.length) {
          const t = sorted[i].ord;
          sorted[i].ord = sorted[j].ord;
          sorted[j].ord = t;
        }
        return undefined as T;
      }
      case "set_mod_category": {
        const m = mockMods.find((x) => x.id === Number(args?.id));
        if (!m) throw "Mod 不存在";
        const cid = args?.categoryId == null ? null : Number(args.categoryId);
        if (cid !== null && !mockCategories.some((c) => c.id === cid)) throw "分类不存在";
        m.category_id = cid;
        for (const c of mockCategories)
          c.mod_count = mockMods.filter((x) => x.category_id === c.id).length;
        return undefined as T;
      }
      case "list_category_mods":
        return structuredClone(mockMods.filter((m) => m.category_id === Number(args?.categoryId))) as T;
      case "list_all_mods":
        return structuredClone(mockMods) as T;
      case "list_uncategorized_mods":
        return [] as T;
      case "read_log":
        return "2026-08-18T10:00:00 INFO LiquiMod starting\n2026-08-18T10:01:00 INFO installed mod 99" as T;
      case "choose_game_exe":
      case "choose_loader_exe": {
        const p = String(args?.path ?? "");
        if (!p.toLowerCase().endsWith(".exe")) throw "请选择 .exe 可执行文件";
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: "auto", character_category_name: "角色", game_exe: null, loader_exe: null } as T;
      }
      case "launch_game":
        throw "未配置游戏路径，请在设置中配置";
      case "launch_loader":
        throw "未配置加载器路径，请在设置中配置";
      default:
        return undefined as T;
    }
  }
  return invoke<T>(cmd, args);
}

export const api = {
  getConfig: () => call<ConfigDto>("get_config"),
  chooseModsDir: (path: string) => call<ConfigDto>("choose_mods_dir", { path }),
  getCharacters: () => call<CharacterSummary[]>("get_characters"),
  listMods: (character: string) => call<ModDto[]>("list_mods", { character }),
  setModEnabled: (id: number, enabled: boolean) =>
    call<void>("set_mod_enabled", { id, enabled }),
  installMod: (path: string, character?: string | null, password?: string | null) =>
    call<InstallResult>("install_mod", { path, character: character ?? null, password: password ?? null }),
  uninstallMod: (id: number) => call<void>("uninstall_mod", { id }),
  listPresets: () => call<PresetDto[]>("list_presets"),
  savePreset: (name: string) => call<PresetDto>("save_preset", { name }),
  applyPreset: (id: number, name: string) =>
    call<ApplyResultDto>("apply_preset", { id, name }),
  deletePreset: (id: number) => call<void>("delete_preset", { id }),
  listPasswords: () => call<string[]>("list_passwords"),
  addPassword: (value: string) => call<void>("add_password", { value }),
  removePassword: (value: string) => call<void>("remove_password", { value }),
  renameMod: (id: number, name: string) => call<void>("rename_mod", { id, name }),
  setAutoEnable: (enabled: boolean) => call<ConfigDto>("set_auto_enable", { enabled }),
  setTheme: (theme: string) => call<ConfigDto>("set_theme", { theme }),
  setCharacterCategoryName: (name: string) => call<ConfigDto>("set_character_category_name", { name }),
  listCategories: () => call<CategoryDto[]>("list_categories"),
  createCategory: (name: string) => call<number>("create_category", { name }),
  renameCategory: (id: number, name: string) => call<void>("rename_category", { id, name }),
  deleteCategory: (id: number) => call<void>("delete_category", { id }),
  moveCategory: (id: number, delta: number) => call<void>("move_category", { id, delta }),
  setModCategory: (id: number, categoryId: number | null) =>
    call<void>("set_mod_category", { id, categoryId }),
  listCategoryMods: (categoryId: number) => call<ModDto[]>("list_category_mods", { categoryId }),
  listAllMods: () => call<ModDto[]>("list_all_mods"),
  listUncategorizedMods: () => call<ModDto[]>("list_uncategorized_mods"),
  readLog: () => call<string>("read_log"),
  chooseGameExe: (path: string) => call<ConfigDto>("choose_game_exe", { path }),
  chooseLoaderExe: (path: string) => call<ConfigDto>("choose_loader_exe", { path }),
  launchGame: () => call<void>("launch_game"),
  launchLoader: () => call<void>("launch_loader"),
};

/// 立绘 URL（SvelteKit files.assets 指向 assets/hsr）。
export function portraitUrl(image: string): string {
  return `/images/${image}`;
}

/// 网格搜索过滤（不区分大小写，匹配显示名与内部名）。
export function filterCharacters(
  list: CharacterSummary[],
  query: string,
): CharacterSummary[] {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter(
    (c) =>
      c.display_name.toLowerCase().includes(q) ||
      c.internal_name.toLowerCase().includes(q),
  );
}
