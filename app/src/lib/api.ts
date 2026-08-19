import { invoke } from "@tauri-apps/api/core";

export interface ConfigDto {
  library_root: string;
  mods_dir: string | null;
  auto_enable: boolean;
  theme: string;
  character_category_name: string;
  game_exe: string | null;
  loader_exe: string | null;
  favorite_characters?: string[];
  work_mode: "play" | "dev";
  injection_delay_ms: number;
  github_token: string;
  github_mirror: string;
  migoto_version?: string | null;
}

export interface MigotoReleaseInfoDto {
  tag_name: string;
  name: string;
  body: string;
  published_at: string | null;
  download_url: string | null;
  asset_name: string | null;
  size_bytes: number | null;
}

export interface LaunchResultDto {
  success: boolean;
  message: string;
  pid: number | null;
}

export interface MigotoInfoDto {
  root: string;
  ini_path: string;
  game_exe: string | null;
  loader_exe: string | null;
  mods_dir: string | null;
}

export interface CharacterSummary {
  internal_name: string;
  display_name: string;
  image: string | null;
  total: number;
  enabled: number;
  keys?: string[];
  element?: string | null;
  rarity?: number | null;
  is_favorite?: boolean;
}

export interface CategoryDto {
  id: number;
  name: string;
  ord: number;
  /** 固定分类标识（lightcone/portrait/scene/npc/other）；undefined = 用户自定义 */
  kind: string | null;
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
  note: string | null;
  cover_image: string | null;
}

export interface ModKeyBindingDto {
  section: string;
  key: string;
  formatted_key: string;
  back: string | null;
  formatted_back: string | null;
  key_type: string | null;
  variable: string | null;
  steps: number | null;
  comment: string | null;
}

export interface ConflictModInfoDto {
  id: number;
  character: string;
  name: string;
}

export interface ConflictReportDto {
  hash: string;
  section: string;
  conflicting_mods: ConflictModInfoDto[];
}

export interface ModImageDto {
  relative_path: string;
  filename: string;
  size_bytes: number;
  data_url: string;
  is_cover: boolean;
  width?: number;
  height?: number;
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
  { internal_name: "Acheron", display_name: "黄泉", image: "acheron.png", total: 3, enabled: 1, element: "Lightning", rarity: 5, is_favorite: false },
  { internal_name: "Firefly", display_name: "流萤", image: "firefly.png", total: 2, enabled: 2, element: "Fire", rarity: 5, is_favorite: false },
  { internal_name: "Castorice", display_name: "遐蝶", image: "castorice.png", total: 1, enabled: 0, element: "Quantum", rarity: 5, is_favorite: false },
  { internal_name: "Cipher", display_name: "赛芙", image: "cipher.png", total: 0, enabled: 0, element: "Quantum", rarity: 5, is_favorite: false },
  { internal_name: "Aglaea", display_name: "阿格莱雅", image: "aglaea.png", total: 5, enabled: 3, element: "Lightning", rarity: 5, is_favorite: false },
  { internal_name: "Anaxa", display_name: "阿纳克萨", image: "anaxa.png", total: 0, enabled: 0, element: "Wind", rarity: 5, is_favorite: false },
  { internal_name: "Archer", display_name: "Archer", image: "archer.png", total: 2, enabled: 1, element: "Quantum", rarity: 4, is_favorite: false },
  { internal_name: "Argenti", display_name: "银枝", image: "argenti.png", total: 0, enabled: 0, element: "Physical", rarity: 5, is_favorite: false },
  { internal_name: "Arlan", display_name: "阿兰", image: "arlan.png", total: 0, enabled: 0, element: "Lightning", rarity: 4, is_favorite: false },
  { internal_name: "Asta", display_name: "艾丝妲", image: "asta.png", total: 1, enabled: 1, element: "Fire", rarity: 4, is_favorite: false },
  { internal_name: "Aventurine", display_name: "砂金", image: "aventurine.png", total: 0, enabled: 0, element: "Imaginary", rarity: 5, is_favorite: false },
  { internal_name: "Bailu", display_name: "白露", image: "bailu.png", total: 0, enabled: 0, element: "Lightning", rarity: 5, is_favorite: false },
  { internal_name: "Others", display_name: "其他", image: null, total: 2, enabled: 0, element: null, rarity: null, is_favorite: false },
];

const mockMods: ModDto[] = [
  { id: 1, name: "Summer Skin", enabled: true, installed_at: 1755000000, thumb: null, size_bytes: 12345678, file_count: 42, path: "C:/mock/Library/mods/Firefly/Summer Skin", category_id: null, note: null, cover_image: null },
  { id: 2, name: "Battle FX+", enabled: false, installed_at: 1755100000, thumb: null, size_bytes: 12345678, file_count: 42, path: "C:/mock/Library/mods/Firefly/Battle FX+", category_id: null, note: null, cover_image: null },
  { id: 3, name: "HD Textures", enabled: false, installed_at: 1755200000, thumb: null, size_bytes: 12345678, file_count: 42, path: "C:/mock/Library/mods/Firefly/HD Textures", category_id: 1, note: null, cover_image: null },
];

const mockPresets: PresetDto[] = [
  { id: 1, name: "日常出战", created_at: 1755000000 },
  { id: 2, name: "截图模式", created_at: 1755100000 },
];

const mockPasswords: string[] = ["1234"];

const mockCategories: CategoryDto[] = [
  { id: 1, name: "光锥", ord: 1, kind: "lightcone", mod_count: 0 },
  { id: 2, name: "立绘", ord: 2, kind: "portrait", mod_count: 0 },
  { id: 3, name: "场景", ord: 3, kind: "scene", mod_count: 0 },
  { id: 4, name: "NPC", ord: 4, kind: "npc", mod_count: 0 },
  { id: 5, name: "其他", ord: 5, kind: "other", mod_count: 0 },
  { id: 9, name: "武器", ord: 6, kind: null, mod_count: 1 },
];

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    switch (cmd) {
      case "get_config":
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: "auto", character_category_name: "角色", game_exe: null, loader_exe: null } as T;
      case "get_characters": {
        const cid = args?.categoryId == null ? null : Number(args.categoryId);
        return structuredClone(mockCharacters).map((c) => ({
          ...c,
          total: cid === null ? c.total : mockMods.filter((m) => m.category_id === cid).length,
          enabled: cid === null ? c.enabled : mockMods.filter((m) => m.category_id === cid && m.enabled).length,
        })) as T;
      }
      case "list_mods": {
        const cid = args?.categoryId == null ? null : Number(args.categoryId);
        const list = cid === null
          ? mockMods.filter((m) => m.category_id === null)
          : mockMods.filter((m) => m.category_id === cid);
        return structuredClone(list) as T;
      }
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
        const c = { id: Math.max(0, ...mockCategories.map((x) => x.id)) + 1, name: n, ord: Math.max(0, ...mockCategories.map((x) => x.ord)) + 1, kind: null, mod_count: 0 };
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
      case "set_mod_note": {
        const m = mockMods.find((x) => x.id === Number(args?.id));
        if (m) m.note = (args?.note as string) || null;
        return undefined as T;
      }
      case "toggle_favorite_character": {
        const name = String(args?.internalName);
        const c = mockCharacters.find((x) => x.internal_name === name);
        if (c) {
          c.is_favorite = !c.is_favorite;
          return c.is_favorite as T;
        }
        return false as T;
      }
      case "read_log":
        return "2026-08-18T10:00:00 INFO LiquiMod starting\n2026-08-18T10:01:00 INFO installed mod 99" as T;
      case "choose_game_exe":
      case "choose_loader_exe": {
        const p = String(args?.path ?? "");
        if (!p.toLowerCase().endsWith(".exe")) throw "请选择 .exe 可执行文件";
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: "auto", character_category_name: "角色", game_exe: null, loader_exe: null } as T;
      }
      case "launch_game":
      case "launch_game_native":
      case "launch_official_launcher":
        throw "未配置游戏路径，请在设置中配置";
      case "launch_loader":
        throw "未配置加载器路径，请在设置中配置";
      case "inspect_3dmigoto_dir":
        return {
          root: String(args?.path ?? ""),
          ini_path: `${String(args?.path ?? "")}/d3dx.ini`,
          game_exe: "D:/Games/Star Rail/Game/StarRail.exe",
          loader_exe: `${String(args?.path ?? "")}/3DMigoto Loader.exe`,
          mods_dir: `${String(args?.path ?? "")}/Mods`,
        } as T;
      case "import_3dmigoto_dir":
        return {
          library_root: "C:/mock/Library",
          mods_dir: `${String(args?.path ?? "")}/Mods`,
          auto_enable: false,
          theme: "auto",
          character_category_name: "角色",
          game_exe: "D:/Games/Star Rail/Game/StarRail.exe",
          loader_exe: `${String(args?.path ?? "")}/3DMigoto Loader.exe`,
        } as T;
      case "get_mod_keys":
        return [
          {
            section: "KeySwapHead",
            key: "VK_SHIFT VK_UP",
            formatted_key: "Shift + ↑",
            back: "VK_SHIFT VK_DOWN",
            formatted_back: "Shift + ↓",
            key_type: "cycle",
            variable: "$swaphead",
            steps: 3,
            comment: "发型切换",
          },
        ] as T;
      case "set_mod_custom_cover":
        return "data:image/jpeg;base64,mocknewthumb" as T;
      case "get_active_conflicts":
        return [] as T;
      case "get_mod_images":
        return [
          {
            relative_path: "preview.png",
            filename: "preview.png",
            size_bytes: 485200,
            data_url: "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='800' height='500' viewBox='0 0 800 500'><rect fill='%236366f1' width='800' height='500'/><text fill='white' font-family='sans-serif' font-size='32' font-weight='bold' x='50%25' y='45%25' text-anchor='middle'>主视觉封面 (800x500)</text><text fill='%23e0e7ff' font-family='sans-serif' font-size='18' x='50%25' y='55%25' text-anchor='middle'>preview.png - ★ 当前封面</text></svg>",
            is_cover: true,
            width: 800,
            height: 500,
          },
          {
            relative_path: "screenshots/battle_01.png",
            filename: "battle_01.png",
            size_bytes: 1245000,
            data_url: "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='1920' height='1080' viewBox='0 0 1920 1080'><rect fill='%23059669' width='1920' height='1080'/><text fill='white' font-family='sans-serif' font-size='48' font-weight='bold' x='50%25' y='45%25' text-anchor='middle'>战斗实机刀光截图 1 (1920x1080)</text><text fill='%23d1fae5' font-family='sans-serif' font-size='24' x='50%25' y='55%25' text-anchor='middle'>screenshots/battle_01.png</text></svg>",
            is_cover: false,
            width: 1920,
            height: 1080,
          },
          {
            relative_path: "screenshots/closeup.jpg",
            filename: "closeup.jpg",
            size_bytes: 840000,
            data_url: "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='1440' height='900' viewBox='0 0 1440 900'><rect fill='%23d97706' width='1440' height='900'/><text fill='white' font-family='sans-serif' font-size='40' font-weight='bold' x='50%25' y='45%25' text-anchor='middle'>面部近景细节展示 (1440x900)</text><text fill='%23fef3c7' font-family='sans-serif' font-size='22' x='50%25' y='55%25' text-anchor='middle'>screenshots/closeup.jpg</text></svg>",
            is_cover: false,
            width: 1440,
            height: 900,
          },
          {
            relative_path: "textures/icon.png",
            filename: "icon.png",
            size_bytes: 120000,
            data_url: "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='512' height='512' viewBox='0 0 512 512'><rect fill='%23db2777' width='512' height='512'/><text fill='white' font-family='sans-serif' font-size='32' font-weight='bold' x='50%25' y='50%25' text-anchor='middle'>UI 头像图标</text></svg>",
            is_cover: false,
            width: 512,
            height: 512,
          },
        ] as T;
      case "set_mod_cover_from_internal":
        return "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='800' height='500' viewBox='0 0 800 500'><rect fill='%23059669' width='800' height='500'/></svg>" as T;
      case "reset_mod_cover":
        return "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='800' height='500' viewBox='0 0 800 500'><rect fill='%236366f1' width='800' height='500'/></svg>" as T;
      case "get_mod_cover_image":
        return "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='1920' height='1080' viewBox='0 0 1920 1080'><rect fill='%236366f1' width='1920' height='1080'/><text fill='white' font-family='sans-serif' font-size='48' font-weight='bold' x='50%25' y='50%25' text-anchor='middle'>4K 高清原图封面 (1920x1080)</text></svg>" as T;
      case "rescan_library":
        return { added: 0, removed: 0 } as T;
      case "clean_cache":
        return 0 as T;
      case "get_diagnostic_status":
        return {
          helper_ready: true,
          game_configured: true,
          loader_configured: true,
          mods_dir_configured: true,
        } as T;
      default:
        return undefined as T;
    }
  }
  return invoke<T>(cmd, args);
}

export const api = {
  getConfig: () => call<ConfigDto>("get_config"),
  chooseModsDir: (path: string) => call<ConfigDto>("choose_mods_dir", { path }),
  getCharacters: (categoryId?: number | null) =>
    call<CharacterSummary[]>("get_characters", { categoryId: categoryId ?? null }),
  listMods: (character: string, categoryId?: number | null) =>
    call<ModDto[]>("list_mods", { character, categoryId: categoryId ?? null }),
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
  launchGame: () => call<LaunchResultDto>("launch_game"),
  launchGameNative: () => call<LaunchResultDto>("launch_game_native"),
  launchOfficialLauncher: () => call<LaunchResultDto>("launch_official_launcher"),
  launchLoader: () => call<void>("launch_loader"),
  inspect3dMigotoDir: (path: string) =>
    call<MigotoInfoDto>("inspect_3dmigoto_dir", { path }),
  import3dMigotoDir: (path: string) =>
    call<ConfigDto>("import_3dmigoto_dir", { path }),
  getModKeys: (id: number) => call<ModKeyBindingDto[]>("get_mod_keys", { id }),
  setModCustomCover: (id: number, imagePath: string) =>
    call<string | null>("set_mod_custom_cover", { id, imagePath }),
  getActiveConflicts: () => call<ConflictReportDto[]>("get_active_conflicts"),
  openModFolder: (id: number) => call<void>("open_mod_folder", { id }),
  openPathInExplorer: (path: string) => call<void>("open_path_in_explorer", { path }),
  triggerRefreshGame: () => call<void>("trigger_refresh_game"),
  getModImages: (id: number) => call<ModImageDto[]>("get_mod_images", { id }),
  setModCoverFromInternal: (id: number, relativePath: string) =>
    call<string>("set_mod_cover_from_internal", { id, relativePath }),
  resetModCover: (id: number) => call<string | null>("reset_mod_cover", { id }),
  getModCoverImage: (id: number) => call<string | null>("get_mod_cover_image", { id }),
  rescanLibrary: () => call<RescanResultDto>("rescan_library"),
  cleanCache: () => call<number>("clean_cache"),
  getDiagnosticStatus: () => call<DiagnosticStatusDto>("get_diagnostic_status"),
  getLocalAssetVersion: () => call<string | null>("get_local_asset_version"),
  checkGameAssetsUpdate: (game = "Honkai") => call<AssetUpdateCheckResultDto>("check_game_assets_update", { game }),
  syncGameAssets: (game = "Honkai") => call<AssetSyncResultDto>("sync_game_assets", { game }),
  getCharacterImageData: (filename: string, game = "Honkai") =>
    call<string | null>("get_character_image_data", { filename, game }),
  setModNote: (id: number, note: string | null) =>
    call<void>("set_mod_note", { id, note }),
  toggleFavoriteCharacter: (internalName: string) =>
    call<boolean>("toggle_favorite_character", { internalName }),
  autoDetectGameExe: () => call<string | null>("auto_detect_game_exe"),
  initMigotoWorkspace: (targetDir: string) =>
    call<string>("init_migoto_workspace", { targetDir }),
  checkMigotoUpdate: () => call<MigotoReleaseInfoDto>("check_migoto_update"),
  installMigotoUpdate: (downloadUrl: string, versionTag?: string) =>
    call<ConfigDto>("install_migoto_update", { downloadUrl, versionTag: versionTag ?? null }),
  switchToManagedMigoto: () => call<ConfigDto>("switch_to_managed_migoto"),
  migrateModsFromOldMigoto: (oldDir: string) =>
    call<MigrateResultDto>("migrate_mods_from_old_migoto", { oldDir }),
  setWorkMode: (mode: "play" | "dev") => call<ConfigDto>("set_work_mode", { mode }),
  setInjectionDelay: (delayMs: number) =>
    call<ConfigDto>("set_injection_delay", { delayMs }),
  setGithubToken: (token: string) => call<ConfigDto>("set_github_token", { token }),
  setGithubMirror: (mirror: string) => call<ConfigDto>("set_github_mirror", { mirror }),
};

export interface MigotoDownloadProgressDto {
  stage: "downloading" | "extracting" | "completed" | "failed";
  percent: number;
  downloaded_bytes: number;
  total_bytes: number | null;
  message: string;
}

export interface MigrateResultDto {
  total_found: number;
  migrated_count: number;
  failed_count: number;
  errors: string[];
}

export interface AssetUpdateCheckResultDto {
  has_update: boolean;
  remote_version: string | null;
  local_version: string | null;
}

export interface AssetSyncProgressDto {
  stage: "checking" | "downloading" | "cleaning" | "completed" | "failed";
  percent: number;
  current_file: string | null;
  downloaded_count: number;
  total_count: number;
  message: string;
}

export interface AssetSyncResultDto {
  success: boolean;
  message: string;
  version: string;
  downloaded_count: number;
  deleted_count: number;
}

export interface RescanResultDto {
  added: number;
  removed: number;
}

export interface DiagnosticStatusDto {
  helper_ready: boolean;
  game_configured: boolean;
  loader_configured: boolean;
  mods_dir_configured: boolean;
}

/// 立绘 URL（SvelteKit files.assets 指向 assets/hsr）。
export function portraitUrl(image: string): string {
  return `/images/${image}`;
}

const characterImageCache = new Map<string, string>();

export async function resolveCharacterImage(filename: string, game = "Honkai"): Promise<string> {
  if (!filename) return "/images/Others.png";
  if (characterImageCache.has(filename)) {
    return characterImageCache.get(filename)!;
  }
  try {
    const data = await api.getCharacterImageData(filename, game);
    if (data) {
      characterImageCache.set(filename, data);
      return data;
    }
  } catch {}
  const fallback = portraitUrl(filename);
  characterImageCache.set(filename, fallback);
  return fallback;
}

export function getCachedCharacterImage(filename: string): string | null {
  return characterImageCache.get(filename) || null;
}

export type CharacterSortOption = "default" | "name" | "mods" | "enabled" | "rarity";

/// 网格搜索与属性过滤（支持显示名、内部名、拼音 Keys 以及属性 Element 过滤）。
export function filterCharacters(
  list: CharacterSummary[],
  query: string,
  elementFilter?: string | null,
): CharacterSummary[] {
  const q = query.trim().toLowerCase();
  return list.filter((c) => {
    // 1. 属性过滤
    if (elementFilter && elementFilter !== "all") {
      if (!c.element || c.element.toLowerCase() !== elementFilter.toLowerCase()) {
        return false;
      }
    }
    // 2. 关键词与拼音检索
    if (!q) return true;
    return (
      c.display_name.toLowerCase().includes(q) ||
      c.internal_name.toLowerCase().includes(q) ||
      c.keys?.some((k) => k.toLowerCase().includes(q))
    );
  });
}

/// 角色多维排序（喜爱角色始终置顶优先）
export function sortCharacters(
  list: CharacterSummary[],
  sort: CharacterSortOption,
  ascending = true,
): CharacterSummary[] {
  const cloned = [...list];
  cloned.sort((a, b) => {
    // 1. 喜爱角色永远置顶
    const favA = a.is_favorite ? 1 : 0;
    const favB = b.is_favorite ? 1 : 0;
    if (favA !== favB) return favB - favA;

    // 2. 次级排序依据
    let diff = 0;
    switch (sort) {
      case "name":
        diff = a.display_name.localeCompare(b.display_name, "zh-CN");
        break;
      case "mods":
        diff = b.total - a.total; // 默认 Mod 数多的在前
        break;
      case "enabled":
        diff = b.enabled - a.enabled; // 已启用的在前
        break;
      case "rarity":
        diff = (b.rarity ?? 0) - (a.rarity ?? 0); // 5星在前
        break;
      case "default":
      default:
        return 0;
    }
    return ascending ? diff : -diff;
  });
  return cloned;
}
