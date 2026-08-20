<script lang="ts">
  import {
    api,
    type CategoryDto,
    type CharacterSummary,
    type ModDto,
    type ModKeyBindingDto,
    type ModImageDto,
  } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import Toggle from "./Toggle.svelte";
  import CategoryMenu from "./CategoryMenu.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { pushEscHandler } from "$lib/esc";

  let {
    mod,
    categories,
    character,
    ontoggle,
    onrename,
    onuninstall,
    onopen,
    onmove,
  }: {
    mod: ModDto | null;
    categories: CategoryDto[];
    character?: CharacterSummary;
    ontoggle: (next: boolean) => void;
    onrename: (name: string) => Promise<boolean>;
    onuninstall: () => Promise<void>;
    onopen: () => void;
    onmove: (categoryId: number | null) => void;
  } = $props();

  let renaming = $state(false);
  let draft = $state("");
  let confirming = $state(false);
  let busy = $state(false);
  let cancelled = $state(false);

  // 🖼️ 现代化大图画廊与 Lightbox 状态
  let lightboxOpen = $state(false);
  let activeImageIndex = $state(0);
  let activeFallbackImage = $state<string | null>(null);
  let zoom = $state(1);
  let pan = $state({ x: 0, y: 0 });
  let isDragging = $state(false);
  let dragStart = $state({ x: 0, y: 0 });

  let keys = $state<ModKeyBindingDto[]>([]);
  let loadingKeys = $state(false);
  let images = $state<ModImageDto[]>([]);
  let loadingImages = $state(false);

  let noteDraft = $state("");
  let savingNote = $state(false);

  // 当 mod 切换时自动重置面板内部状态并拉取热键绑定与内置图集
  $effect(() => {
    confirming = false;
    renaming = false;
    lightboxOpen = false;
    activeFallbackImage = null;
    noteDraft = mod?.note || "";
    if (mod) {
      loadingKeys = true;
      api.getModKeys(mod.id)
        .then((res) => {
          keys = res;
        })
        .catch(() => {
          keys = [];
        })
        .finally(() => {
          loadingKeys = false;
        });

      loadingImages = true;
      api.getModImages(mod.id)
        .then((res) => {
          images = res;
        })
        .catch(() => {
          images = [];
        })
        .finally(() => {
          loadingImages = false;
        });
    } else {
      keys = [];
      images = [];
    }
  });

  $effect(() => {
    if (lightboxOpen) {
      return pushEscHandler(() => {
        lightboxOpen = false;
        return true;
      });
    }
  });

  $effect(() => {
    if (renaming) {
      return pushEscHandler(() => {
        cancelRename();
        return true;
      });
    }
  });

  $effect(() => {
    if (confirming) {
      return pushEscHandler(() => {
        confirming = false;
        return true;
      });
    }
  });

  function fmtSize(b: number): string {
    if (b < 0) return "—";
    if (b < 1024) return `${b} B`;
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(0)} KB`;
    if (b < 1024 * 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`;
    return `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  function fmtDate(ts: number): string {
    const d = new Date(ts * 1000);
    return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`;
  }

  function startRename() {
    if (!mod) return;
    draft = mod.name;
    renaming = true;
  }

  async function commitRename() {
    if (!mod || cancelled) {
      cancelled = false;
      return;
    }
    const v = draft.trim();
    if (!v || v === mod.name || busy) {
      renaming = false;
      return;
    }
    busy = true;
    try {
      const ok = await onrename(v);
      if (ok) renaming = false;
    } finally {
      busy = false;
    }
  }

  function cancelRename() {
    cancelled = true;
    renaming = false;
  }

  function onInputKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      commitRename();
    } else if (e.key === "Escape") {
      cancelRename();
    }
  }

  async function confirmUninstall() {
    if (!mod || busy) return;
    busy = true;
    try {
      await onuninstall();
      confirming = false;
    } finally {
      busy = false;
    }
  }

  async function openModFolder() {
    if (!mod) return;
    try {
      onopen();
      await api.openModFolder(mod.id);
    } catch {
      // 容错
    }
  }

  async function pickCustomCover() {
    if (!mod || busy) return;
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "图片文件", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif", "avif"] }],
        title: "选择 Mod 封面图",
      });
      if (typeof selected === "string") {
        busy = true;
        const newThumb = await api.setModCustomCover(mod.id, selected);
        if (newThumb && mod) {
          mod.thumb = newThumb;
        }
        const updatedImgs = await api.getModImages(mod.id);
        images = updatedImgs;
        toast("已更换 Mod 封面");
      }
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function setCoverFromGallery(img: ModImageDto) {
    if (!mod || busy) return;
    busy = true;
    try {
      const newThumb = await api.setModCoverFromInternal(mod.id, img.relative_path);
      if (newThumb && mod) {
        mod.thumb = newThumb;
      }
      const updatedImgs = await api.getModImages(mod.id);
      images = updatedImgs;
      toast(`已将「${img.filename}」设为 Mod 封面`);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function resetCover() {
    if (!mod || busy) return;
    busy = true;
    try {
      const newThumb = await api.resetModCover(mod.id);
      if (mod) {
        mod.thumb = newThumb;
      }
      const updatedImgs = await api.getModImages(mod.id);
      images = updatedImgs;
      toast("已恢复 Mod 默认封面");
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function openCoverLightbox() {
    if (!mod) return;
    zoom = 1;
    pan = { x: 0, y: 0 };
    if (images.length > 0) {
      const coverIdx = images.findIndex((img) => img.is_cover);
      activeImageIndex = coverIdx >= 0 ? coverIdx : 0;
      activeFallbackImage = null;
      lightboxOpen = true;
    } else {
      try {
        const hdCover = await api.getModCoverImage(mod.id);
        activeFallbackImage = hdCover || mod.thumb;
      } catch {
        activeFallbackImage = mod.thumb;
      }
      lightboxOpen = true;
    }
  }

  function openLightboxByIndex(idx: number) {
    activeImageIndex = idx;
    activeFallbackImage = null;
    zoom = 1;
    pan = { x: 0, y: 0 };
    lightboxOpen = true;
  }

  function nextImage() {
    if (images.length <= 1) return;
    activeImageIndex = (activeImageIndex + 1) % images.length;
    zoom = 1;
    pan = { x: 0, y: 0 };
  }

  function prevImage() {
    if (images.length <= 1) return;
    activeImageIndex = (activeImageIndex - 1 + images.length) % images.length;
    zoom = 1;
    pan = { x: 0, y: 0 };
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const delta = e.deltaY < 0 ? 0.15 : -0.15;
    const nextZoom = Math.min(5, Math.max(0.5, zoom + delta));
    zoom = nextZoom;
    if (zoom <= 1) {
      pan = { x: 0, y: 0 };
    }
  }

  function handleDblClick() {
    if (zoom === 1) {
      zoom = 2;
    } else {
      zoom = 1;
      pan = { x: 0, y: 0 };
    }
  }

  let rafId: number | null = null;

  function handleMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    if (zoom > 1) {
      isDragging = true;
      dragStart = { x: e.clientX - pan.x, y: e.clientY - pan.y };

      const onGlobalMouseMove = (moveEvent: MouseEvent) => {
        if (!isDragging) return;
        if (rafId !== null) cancelAnimationFrame(rafId);
        rafId = requestAnimationFrame(() => {
          pan = {
            x: moveEvent.clientX - dragStart.x,
            y: moveEvent.clientY - dragStart.y,
          };
        });
      };

      const onGlobalMouseUp = () => {
        if (rafId !== null) cancelAnimationFrame(rafId);
        isDragging = false;
        window.removeEventListener("mousemove", onGlobalMouseMove);
        window.removeEventListener("mouseup", onGlobalMouseUp);
      };

      window.addEventListener("mousemove", onGlobalMouseMove, { passive: true });
      window.addEventListener("mouseup", onGlobalMouseUp, { once: true });
    }
  }

  function handleLightboxKeydown(e: KeyboardEvent) {
    if (!lightboxOpen) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();
      lightboxOpen = false;
      return;
    }
    if (e.key === "ArrowRight" || e.key === "d" || e.key === "D") {
      e.preventDefault();
      e.stopPropagation();
      nextImage();
    } else if (e.key === "ArrowLeft" || e.key === "a" || e.key === "A") {
      e.preventDefault();
      e.stopPropagation();
      prevImage();
    } else if (e.key === "+" || e.key === "=") {
      e.preventDefault();
      e.stopPropagation();
      zoom = Math.min(5, zoom + 0.25);
    } else if (e.key === "-") {
      e.preventDefault();
      e.stopPropagation();
      zoom = Math.max(0.5, zoom - 0.25);
      if (zoom <= 1) pan = { x: 0, y: 0 };
    } else if (e.key === "0") {
      e.preventDefault();
      e.stopPropagation();
      zoom = 1;
      pan = { x: 0, y: 0 };
    }
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        if (node.parentNode) {
          node.parentNode.removeChild(node);
        }
      },
    };
  }

  function focusOn(el: HTMLElement) {
    el.focus();
  }

  async function saveNote() {
    if (!mod) return;
    const v = noteDraft.trim();
    if (v === (mod.note || "")) return;
    savingNote = true;
    try {
      await api.setModNote(mod.id, v || null);
      mod.note = v || null;
    } catch {
      // 容错
    } finally {
      savingNote = false;
    }
  }

  let currentCategoryName = $derived.by(() => {
    if (!mod || mod.category_id == null) return character?.display_name || "默认角色";
    const c = categories.find((x) => x.id === mod.category_id);
    return c ? c.name : "未分类";
  });

  let currentActiveImage = $derived.by(() => {
    if (images.length > 0 && activeImageIndex < images.length) {
      return images[activeImageIndex];
    }
    return null;
  });
</script>

<svelte:window onkeydown={handleLightboxKeydown} />

<div class="glass radius-panel p-6 flex flex-col gap-5 h-full min-h-0 overflow-y-auto">
  {#if mod}
    <!-- 1. 大预览图展示区（带双层极光环境光与立体悬浮海报设计） -->
    <div
      class="group relative w-full h-[260px] min-h-[200px] max-h-[340px] radius-card overflow-hidden flex items-center justify-center p-3 shrink-0 cursor-pointer select-none"
      style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)"
      role="button"
      tabindex="0"
      aria-label="查看高清全屏大图"
      onclick={openCoverLightbox}
      onkeydown={(e) => (e.key === "Enter" || e.key === " ") && openCoverLightbox()}
    >
      {#if mod.thumb}
        <!-- 双层极光流体环境光背景（彻底消除死板灰色留白） -->
        <img
          src={mod.thumb}
          alt=""
          class="absolute inset-0 w-full h-full object-cover blur-3xl scale-150 opacity-65 saturate-150 brightness-90 pointer-events-none"
          draggable="false"
        />
        <div class="absolute inset-0 bg-black/20 pointer-events-none"></div>

        <!-- 主图主体（自适应居中悬浮立体卡片） -->
        <img
          src={mod.thumb}
          alt={mod.name}
          class="relative max-h-full max-w-full object-contain rounded-xl shadow-[0_12px_32px_rgba(0,0,0,0.45)] ring-1 ring-white/25 z-10 transition-all duration-300 group-hover:scale-[1.03] group-hover:shadow-[0_16px_40px_rgba(0,0,0,0.6)]"
          draggable="false"
        />
        <!-- 状态角标 -->
        <div class="absolute top-2.5 right-2.5 z-20">
          <span
            class="px-2.5 py-1 radius-pill text-xs font-semibold backdrop-blur-md shadow-sm transition-all"
            style={mod.enabled
              ? "background: rgba(34, 197, 94, 0.2); color: #22c55e; box-shadow: inset 0 0 0 0.5px rgba(34, 197, 94, 0.5)"
              : "background: rgba(0, 0, 0, 0.4); color: rgba(255, 255, 255, 0.8)"}
          >
            {mod.enabled ? "已启用" : "未启用"}
          </span>
        </div>
      {:else}
        <div class="flex flex-col items-center gap-2 text-secondary opacity-60 z-10">
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
            <circle cx="8.5" cy="8.5" r="1.5"/>
            <polyline points="21 15 16 10 5 21"/>
          </svg>
          <span class="text-xs">暂无预览图</span>
        </div>
      {/if}

      <!-- 悬浮操作栏（换封面 / 放大提示） -->
      <div
        class="absolute bottom-2.5 right-2.5 z-20 flex items-center gap-1.5 opacity-0 group-hover:opacity-100 transition-opacity"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
        role="toolbar"
        tabindex="-1"
      >
        <button
          class="h-7 px-2.5 radius-pill text-xs font-medium backdrop-blur-md cursor-pointer flex items-center gap-1 shadow-sm transition-all hover:scale-105"
          style="background: rgba(0,0,0,0.75); color: #fff"
          title="从本地选择图片更换封面"
          onclick={pickCustomCover}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="17 8 12 3 7 8"/>
            <line x1="12" y1="3" x2="12" y2="15"/>
          </svg>
          本地封面
        </button>
        {#if mod.cover_image}
          <button
            class="h-7 px-2.5 radius-pill text-xs font-medium backdrop-blur-md cursor-pointer flex items-center gap-1 shadow-sm transition-all hover:scale-105"
            style="background: rgba(0,0,0,0.75); color: #fff"
            title="恢复默认封面探测"
            onclick={resetCover}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/>
              <path d="M21 3v5h-5"/>
              <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/>
              <path d="M8 16H3v5"/>
            </svg>
            恢复默认
          </button>
        {/if}
      </div>
    </div>

    <!-- 2. 标题、路径与启用开关 -->
    <div class="flex items-start justify-between gap-3 shrink-0">
      <div class="flex-1 min-w-0">
        {#if renaming}
          <div class="flex items-center gap-1.5">
            <input
              use:focusOn
              bind:value={draft}
              type="text"
              class="glass radius-pill px-3 h-8 text-base font-bold flex-1 min-w-0 outline-none"
              style="box-shadow: inset 0 0 0 1.5px var(--accent)"
              onkeydown={onInputKeydown}
              onblur={commitRename}
            />
            <button
              class="glass radius-pill w-8 h-8 grid place-items-center text-xs cursor-pointer text-secondary shrink-0"
              title="取消 (Esc)"
              onmousedown={(e) => e.preventDefault()}
              onclick={cancelRename}
            >
              ✕
            </button>
          </div>
        {:else}
          <div class="flex items-center gap-2 group/title">
            <h2 class="text-xl font-bold tracking-tight truncate select-text" title={mod.name}>
              {mod.name}
            </h2>
            <button
              class="glass radius-pill w-8 h-8 grid place-items-center opacity-0 group-hover/title:opacity-100 transition-opacity text-secondary hover:text-[var(--text)] cursor-pointer shrink-0"
              title="重命名"
              aria-label="重命名"
              onclick={startRename}
            >
              <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
                <path d="M8.6 2.2 10.8 4.4 4.7 10.5l-2.9.7.7-2.9 6.1-6.1Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
              </svg>
            </button>
          </div>
          <div class="flex items-center gap-1.5 mt-0.5 min-w-0">
            <p class="text-xs text-secondary truncate flex-1 min-w-0" title={mod.path}>
              {mod.path}
            </p>
            <button
              class="glass radius-pill h-5 px-2 text-[10px] text-secondary hover:text-[var(--text)] flex items-center gap-1 cursor-pointer shrink-0 transition-colors"
              title="在 Windows 资源管理器中打开此文件夹"
              onclick={openModFolder}
            >
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
                <polyline points="15 3 21 3 21 9"/>
                <line x1="10" y1="14" x2="21" y2="3"/>
              </svg>
              打开
            </button>
          </div>
        {/if}
      </div>

      <div class="shrink-0 flex items-center gap-2">
        <Toggle
          checked={mod.enabled}
          ariaLabel={`启用 ${mod.name}`}
          onchange={(next) => ontoggle(next)}
        />
      </div>
    </div>

    <!-- 3. 元数据信息网格（2x2 统计卡片） -->
    <div class="grid grid-cols-2 gap-2.5 shrink-0">
      <div class="p-3 radius-card flex flex-col gap-0.5" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)">
        <span class="text-[11px] text-secondary font-medium">占用体积</span>
        <span class="text-sm font-semibold">{fmtSize(mod.size_bytes)}</span>
      </div>
      <div class="p-3 radius-card flex flex-col gap-0.5" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)">
        <span class="text-[11px] text-secondary font-medium">文件数量</span>
        <span class="text-sm font-semibold">{mod.file_count < 0 ? "—" : `${mod.file_count} 个文件`}</span>
      </div>
      <div class="p-3 radius-card flex flex-col gap-0.5" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)">
        <span class="text-[11px] text-secondary font-medium">安装日期</span>
        <span class="text-sm font-semibold">{fmtDate(mod.installed_at)}</span>
      </div>
      <div class="p-3 radius-card flex flex-col gap-0.5" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)">
        <span class="text-[11px] text-secondary font-medium">
          {mod.category_id == null ? "归属角色" : "所属分类"}
        </span>
        <span class="text-sm font-semibold truncate" title={currentCategoryName}>
          {currentCategoryName}
        </span>
      </div>
    </div>

    <!-- 4. 📝 Mod 备忘与备注 -->
    <div class="flex flex-col gap-2 p-3.5 radius-card shrink-0" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)">
      <div class="flex items-center justify-between">
        <span class="text-xs font-semibold text-secondary flex items-center gap-1.5">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
          </svg>
          Mod 备忘 / 备注
        </span>
        {#if savingNote}
          <span class="text-[10px] text-[var(--accent)] font-medium">已保存</span>
        {:else}
          <span class="text-[10px] text-secondary">失焦或回车自动保存</span>
        {/if}
      </div>
      <textarea
        bind:value={noteDraft}
        placeholder="添加自定义备忘（如作者来源、特殊按键说明、注意事项等）..."
        rows="2"
        class="w-full bg-[var(--input-bg)] radius-card p-2.5 text-xs text-[var(--text)] placeholder:text-secondary/60 outline-none resize-none focus:ring-1 focus:ring-[var(--accent)] transition-all"
        onblur={saveNote}
        onkeydown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            saveNote();
            (e.target as HTMLElement).blur();
          }
        }}
      ></textarea>
    </div>

    <!-- 5. 🖼️ Mod 内置图集画廊（全量子目录自动递归检索） -->
    <div class="flex flex-col gap-2 p-3.5 radius-card shrink-0" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)">
      <div class="flex items-center justify-between">
        <span class="text-xs font-semibold text-secondary flex items-center gap-1.5">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
            <circle cx="8.5" cy="8.5" r="1.5"/>
            <polyline points="21 15 16 10 5 21"/>
          </svg>
          内置图集画廊 {loadingImages ? "（扫描中…）" : `(${images.length})`}
        </span>
        {#if images.length > 0}
          <span class="text-[10px] text-secondary">点击查看高清大图 / 滚轮缩放</span>
        {/if}
      </div>

      {#if images.length > 0}
        <div class="grid grid-cols-4 gap-2 mt-1">
          {#each images as img, i (img.relative_path || `${img.filename}_${i}`)}
            <div
              class="group/img relative aspect-square radius-card overflow-hidden cursor-pointer select-none"
              style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: rgba(0,0,0,0.03)"
              role="button"
              tabindex="0"
              title={`${img.filename}${img.width && img.height ? ` (${img.width}×${img.height})` : ""} - 点击放大查看`}
              onclick={() => openLightboxByIndex(i)}
              onkeydown={(e) => (e.key === "Enter" || e.key === " ") && openLightboxByIndex(i)}
            >
              <img src={img.data_url} alt={img.filename} class="w-full h-full object-cover transition-transform group-hover/img:scale-105" />

              <!-- 当前封面角标 -->
              {#if img.is_cover}
                <div class="absolute top-1 left-1 z-10">
                  <span class="text-[9px] px-1.5 py-0.5 radius-pill font-bold bg-amber-500 text-black shadow-md flex items-center gap-0.5">
                    ★ 封面
                  </span>
                </div>
              {/if}

              <!-- 悬停信息浮层 -->
              <div class="absolute inset-0 bg-black/60 opacity-0 group-hover/img:opacity-100 transition-opacity flex flex-col justify-between p-1.5 z-20">
                <span class="text-[9px] text-white font-mono truncate">{img.filename}</span>
                <div class="flex items-center justify-between gap-1">
                  {#if img.width && img.height}
                    <span class="text-[8px] text-white/70 font-mono">{img.width}×{img.height}</span>
                  {:else}
                    <span></span>
                  {/if}
                  {#if !img.is_cover}
                    <button
                      class="radius-pill text-[9px] py-0.5 px-1.5 bg-white text-black font-semibold shadow hover:scale-105 transition-transform"
                      onclick={(e) => {
                        e.stopPropagation();
                        setCoverFromGallery(img);
                      }}
                    >
                      设为封面
                    </button>
                  {/if}
                </div>
              </div>
            </div>
          {/each}
        </div>
      {:else if !loadingImages}
        <div class="flex flex-col items-center justify-center py-4 text-secondary/60 gap-1 select-none">
          <p class="text-xs">该 Mod 目录内暂无其他图片</p>
          <button
            class="text-[11px] text-[var(--accent)] hover:underline cursor-pointer"
            onclick={pickCustomCover}
          >
            从本地选择一张封面
          </button>
        </div>
      {/if}
    </div>

    <!-- 6. 🎮 动态切换热键模块 -->
    <div class="flex flex-col gap-2 p-3.5 radius-card shrink-0" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)">
      <div class="flex items-center justify-between">
        <span class="text-xs font-semibold text-secondary flex items-center gap-1.5">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="2" y="6" width="20" height="12" rx="2"/>
            <path d="M6 12h4m-2-2v4m9-2h.01m3-2h.01"/>
          </svg>
          动态切换热键
        </span>
        {#if loadingKeys}
          <span class="text-[11px] text-secondary">扫描中…</span>
        {:else if keys.length > 0}
          <span class="text-[11px] text-secondary font-medium">{keys.length} 组按键</span>
        {/if}
      </div>

      {#if keys.length > 0}
        <div class="flex flex-col gap-1.5 mt-1">
          {#each keys as k, i (`${k.section}_${k.key}_${i}`)}
            <div class="flex items-center justify-between gap-2 text-xs py-1.5 px-2.5 rounded-lg transition-colors hover:bg-[var(--item-hover)]" style="background: var(--input-bg)">
              <div class="flex items-center gap-1.5 min-w-0 flex-1">
                {#if k.variable}
                  <span class="font-mono font-bold text-[var(--accent)] text-[12px] truncate" title={`控制变量：${k.variable}`}>
                    {k.variable}
                  </span>
                  {#if k.comment && k.comment !== k.variable}
                    <span class="text-secondary text-[11px] truncate opacity-90">
                      {k.comment}
                    </span>
                  {/if}
                {:else if k.comment}
                  <span class="font-semibold text-[var(--text)] truncate">{k.comment}</span>
                  <span class="text-secondary text-[10px] font-mono opacity-60 truncate">({k.section})</span>
                {:else}
                  <span class="font-mono font-semibold text-[var(--text)] truncate">{k.section}</span>
                {/if}
                {#if k.steps}
                  <span class="text-[10px] text-secondary px-1.5 py-0.2 rounded-full shrink-0 font-medium" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
                    {k.steps} 档
                  </span>
                {/if}
              </div>
              <div class="flex items-center gap-1 shrink-0 font-mono font-semibold">
                <span class="px-2 py-0.5 radius-pill text-[11px] accent-fill accent-text shadow-sm tracking-tight">
                  {k.formatted_key}
                </span>
                {#if k.formatted_back}
                  <span class="text-secondary text-[10px]">/</span>
                  <span class="px-2 py-0.5 radius-pill text-[11px] accent-fill accent-text shadow-sm tracking-tight">
                    {k.formatted_back}
                  </span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <p class="text-xs text-secondary py-1">此 Mod 为静态外观，未配置 INI 动态切换热键。</p>
      {/if}
    </div>

    <!-- 7. 快捷操作工具栏 -->
    <div class="flex flex-col gap-2 mt-auto pt-4 border-t border-[var(--glass-stroke)] shrink-0">
      {#if confirming}
        <div class="p-3 rounded-xl flex flex-col gap-2" style="background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.3)">
          <p class="text-xs text-red-500 font-medium">确定要彻底卸载此 Mod 吗？所有相关文件将被物理删除。</p>
          <div class="flex items-center gap-2 justify-end">
            <button
              class="radius-pill h-8 px-3.5 text-xs font-medium text-white cursor-pointer disabled:opacity-50"
              style="background: var(--danger)"
              disabled={busy}
              onclick={confirmUninstall}
            >
              确定删除
            </button>
            <button
              class="glass radius-pill h-8 px-3.5 text-xs font-medium cursor-pointer"
              onclick={() => (confirming = false)}
            >
              取消
            </button>
          </div>
        </div>
      {:else}
        <div class="flex items-center justify-between gap-2">
          <div class="flex items-center gap-2">
            <CategoryMenu {categories} current={mod.category_id} label="移到分类" onpick={(catId: number | null) => onmove(catId)} />
            <button
              class="glass radius-pill h-8 px-3 text-xs text-secondary hover:text-[var(--text)] cursor-pointer flex items-center gap-1.5 transition-colors"
              title="在文件资源管理器中打开"
              onclick={openModFolder}
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
              打开目录
            </button>
          </div>
          <button
            class="radius-pill h-8 px-3 text-xs font-medium text-red-500 hover:bg-red-500/10 cursor-pointer transition-colors"
            title="彻底删除此 Mod"
            onclick={() => (confirming = true)}
          >
            卸载
          </button>
        </div>
      {/if}
    </div>
  {:else}
    <!-- 空状态提示 -->
    <div class="flex flex-col items-center justify-center flex-1 text-secondary gap-3 py-16 select-none opacity-60">
      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
        <line x1="9" y1="3" x2="9" y2="21"/>
      </svg>
      <p class="text-sm font-medium">未选中 Mod</p>
    </div>
  {/if}
</div>

<!-- 🌟 现代专业级画廊 Lightbox 查看器（纯暗房影院模式，亮/暗主题完全一致） -->
{#if lightboxOpen}
  {@const currentImg = currentActiveImage}
  {@const imgSrc = currentImg ? currentImg.data_url : activeFallbackImage}
  {#if imgSrc}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      use:portal
      class="fixed inset-0 w-screen h-screen z-[9999] bg-neutral-950/95 backdrop-blur-2xl flex flex-col justify-between select-none overflow-hidden"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
    >
      <!-- 1. 顶部悬浮工具栏 -->
      <div class="relative z-30 flex items-center justify-between p-4 px-6 bg-gradient-to-b from-black/90 via-black/50 to-transparent">
        <!-- 左侧信息区 -->
        <div class="flex items-center gap-3 text-white">
          {#if images.length > 1}
            <span class="px-2.5 py-1 radius-pill text-xs font-mono font-bold bg-white/15 backdrop-blur-md text-white border border-white/10">
              {activeImageIndex + 1} / {images.length}
            </span>
          {/if}
          <div class="flex flex-col">
            <span class="text-sm font-semibold truncate max-w-[280px] sm:max-w-md text-white" title={currentImg?.filename || mod?.name}>
              {currentImg?.filename || mod?.name}
            </span>
            <div class="flex items-center gap-2 text-[11px] text-white/70">
              {#if currentImg?.width && currentImg?.height}
                <span>{currentImg.width} × {currentImg.height}</span>
                <span>•</span>
              {/if}
              {#if currentImg?.size_bytes}
                <span>{fmtSize(currentImg.size_bytes)}</span>
              {/if}
              {#if currentImg?.is_cover}
                <span class="text-amber-400 font-bold">★ 当前封面</span>
              {/if}
            </div>
          </div>
        </div>

        <!-- 中间缩放控制 -->
        <div class="hidden md:flex items-center gap-1 bg-neutral-900/90 border border-white/15 px-2 py-1 radius-pill shadow-xl text-white">
          <button
            class="w-7 h-7 grid place-items-center text-white/80 hover:text-white hover:bg-white/15 rounded cursor-pointer transition-colors"
            title="缩小 (-)"
            onclick={() => {
              zoom = Math.max(0.5, zoom - 0.25);
              if (zoom <= 1) pan = { x: 0, y: 0 };
            }}
          >
            -
          </button>
          <button
            class="px-2 h-7 text-xs font-mono text-white/90 hover:text-white hover:bg-white/15 rounded cursor-pointer transition-colors"
            title="双击图片也可快速缩放"
            onclick={handleDblClick}
          >
            {Math.round(zoom * 100)}%
          </button>
          <button
            class="w-7 h-7 grid place-items-center text-white/80 hover:text-white hover:bg-white/15 rounded cursor-pointer transition-colors"
            title="放大 (+)"
            onclick={() => (zoom = Math.min(5, zoom + 0.25))}
          >
            +
          </button>
          <button
            class="px-2 h-7 text-xs text-white/80 hover:text-white hover:bg-white/15 rounded cursor-pointer transition-colors"
            title="重置缩放 (0)"
            onclick={() => {
              zoom = 1;
              pan = { x: 0, y: 0 };
            }}
          >
            复位
          </button>
        </div>

        <!-- 右侧操作栏 -->
        <div class="flex items-center gap-2">
          {#if currentImg && !currentImg.is_cover}
            <button
              class="h-8 px-3.5 radius-pill text-xs font-semibold bg-white text-black hover:bg-white/90 cursor-pointer flex items-center gap-1 shadow-lg transition-transform hover:scale-105"
              onclick={() => setCoverFromGallery(currentImg)}
            >
              ★ 设为封面
            </button>
          {:else if mod?.cover_image}
            <button
              class="h-8 px-3.5 radius-pill text-xs font-medium bg-neutral-900/90 border border-white/15 text-white hover:bg-neutral-800 cursor-pointer flex items-center gap-1 transition-transform hover:scale-105"
              onclick={resetCover}
            >
              ↺ 恢复默认
            </button>
          {/if}

          <button
            class="w-8 h-8 radius-pill bg-neutral-900/90 border border-white/15 grid place-items-center text-white/80 hover:text-white hover:bg-neutral-800 cursor-pointer transition-transform hover:scale-105"
            title="在文件夹中打开此 Mod"
            onclick={openModFolder}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
          </button>

          <button
            class="w-8 h-8 radius-pill bg-neutral-900/90 border border-white/15 grid place-items-center text-white/90 hover:text-red-400 hover:bg-neutral-800 cursor-pointer transition-transform hover:scale-105"
            title="关闭 (Esc)"
            onclick={() => (lightboxOpen = false)}
          >
            ✕
          </button>
        </div>
      </div>

      <!-- 2. 中间大图展示区（支持左右切换、滚轮无级缩放、双击与拖拽） -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="relative flex-1 flex items-center justify-center overflow-hidden p-6 select-none"
        onwheel={handleWheel}
        onmousedown={handleMouseDown}
        ondblclick={handleDblClick}
        style="cursor: {zoom > 1 ? (isDragging ? 'grabbing' : 'grab') : 'zoom-in'}"
      >
        <!-- 左右翻页大箭头按钮 -->
        {#if images.length > 1}
          <button
            class="absolute left-6 z-30 w-12 h-12 radius-pill bg-neutral-900/90 border border-white/15 text-white hover:bg-neutral-800 hover:scale-110 active:scale-95 transition-all grid place-items-center cursor-pointer shadow-2xl"
            title="上一张 (← 或 A)"
            onclick={(e) => {
              e.stopPropagation();
              prevImage();
            }}
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="15 18 9 12 15 6"/>
            </svg>
          </button>

          <button
            class="absolute right-6 z-30 w-12 h-12 radius-pill bg-neutral-900/90 border border-white/15 text-white hover:bg-neutral-800 hover:scale-110 active:scale-95 transition-all grid place-items-center cursor-pointer shadow-2xl"
            title="下一张 (→ 或 D)"
            onclick={(e) => {
              e.stopPropagation();
              nextImage();
            }}
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="9 18 15 12 9 6"/>
            </svg>
          </button>
        {/if}

        <!-- 背景极光氛围漫反射 -->
        <img
          src={imgSrc}
          alt=""
          class="absolute inset-0 w-full h-full object-cover blur-3xl scale-125 opacity-25 pointer-events-none"
          draggable="false"
        />

        <!-- 高清图像主体（自适应竖屏与横屏） -->
        <img
          src={imgSrc}
          alt={currentImg?.filename || "大图预览"}
          class="relative max-w-[92vw] max-h-[82vh] object-contain rounded-lg drop-shadow-2xl pointer-events-none select-none will-change-transform z-10"
          style="transform: translate3d({pan.x}px, {pan.y}px, 0) scale({zoom}); transform-origin: center center; transition: {isDragging ? 'none' : 'transform 0.15s cubic-bezier(0.2, 0, 0, 1)'};"
          draggable="false"
        />
      </div>

      <!-- 3. 底部图集缩略图导航条（Strip） -->
      {#if images.length > 1}
        <div class="relative z-30 p-3 px-6 bg-gradient-to-t from-black/90 via-black/50 to-transparent flex items-center justify-center">
          <div class="flex items-center gap-2 overflow-x-auto max-w-4xl p-1.5 radius-pill bg-neutral-900/90 border border-white/15 shadow-xl">
            {#each images as thumbImg, idx (thumbImg.relative_path || idx)}
              <button
                class="relative w-12 h-12 radius-card overflow-hidden shrink-0 cursor-pointer transition-all hover:scale-105"
                style={idx === activeImageIndex
                  ? "box-shadow: 0 0 0 2px var(--accent), 0 0 12px var(--accent); opacity: 1;"
                  : "opacity: 0.45;"}
                title={thumbImg.filename}
                onclick={() => openLightboxByIndex(idx)}
              >
                <img src={thumbImg.data_url} alt="" class="w-full h-full object-cover" />
                {#if thumbImg.is_cover}
                  <span class="absolute top-0.5 right-0.5 text-[8px] text-amber-400 font-bold">★</span>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
{/if}
