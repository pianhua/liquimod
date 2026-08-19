<script lang="ts">
  import {
    dismissInstall,
    retryInstall,
    startInstallWithCategory,
    submitInstallPassword,
    undoInstall,
    type InstallJob,
  } from "$lib/install.svelte";
  import type { CategoryDto, CharacterSummary } from "$lib/api";

  let {
    jobs,
    characters,
    categories,
    onInstalled,
  }: {
    jobs: InstallJob[];
    characters: CharacterSummary[];
    categories: CategoryDto[];
    onInstalled: () => void;
  } = $props();

  let passwords = $state<Record<number, string>>({});

  // 每个待分类 job 的选择状态：大类和角色
  let pick = $state<Record<number, { scope: string | null; character: string | null }>>({});

  /** 固定分类（kind 非空），用于「选分类」面板的大类选项。 */
  let fixedTypes = $derived(
    categories.filter((c) => c.kind != null).sort((a, b) => a.ord - b.ord),
  );

  function displayName(internal: string): string {
    return characters.find((c) => c.internal_name === internal)?.display_name ?? internal;
  }

  function scopeOf(job: InstallJob): string | null {
    return pick[job.id]?.scope ?? null;
  }
  function charPicked(job: InstallJob): string | null {
    return pick[job.id]?.character ?? null;
  }

  function setScope(job: InstallJob, scope: string | null) {
    pick[job.id] = { scope, character: null };
  }
  function setChar(job: InstallJob, c: string) {
    pick[job.id] = { scope: "character", character: c };
  }

  /** 确认分类并开始安装。 */
  function confirmPick(job: InstallJob) {
    const s = scopeOf(job);
    if (s === "character") {
      const c = charPicked(job);
      if (c) startInstallWithCategory(job, c, onInstalled);
    } else if (s) {
      startInstallWithCategory(job, s, onInstalled);
    }
    delete pick[job.id];
  }
</script>

{#if jobs.length > 0}
  <div class="install-overlay fixed bottom-6 inset-x-0 z-50 flex justify-center pointer-events-none" aria-live="polite">
    <div class="glass radius-panel pointer-events-auto w-[440px] max-w-[90vw] px-5 py-4 flex flex-col gap-3 max-h-[70vh] overflow-y-auto"
      style="box-shadow: var(--shadow-lift)">
      {#each jobs as job (job.id)}
        <div class="flex items-center gap-3 min-h-9">
          {#if job.stage === "installing"}
            <span class="spinner shrink-0"></span>
          {/if}
          <span class="text-sm font-medium truncate flex-1 min-w-0">{job.fileName}</span>

          {#if job.stage === "pick-category"}
            <span class="text-sm text-secondary shrink-0">选择分类…</span>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => dismissInstall(job)}
            >关闭</button>
          {:else if job.stage === "installing"}
            <span class="text-sm text-secondary shrink-0">正在安装…</span>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => {
                delete passwords[job.id];
                dismissInstall(job);
              }}
            >关闭</button>
          {:else if job.stage === "needs-password"}
            <input
              class="glass radius-pill px-3 h-8 text-sm w-32 outline-none bg-transparent text-[var(--text)] placeholder:text-secondary"
              placeholder="压缩包密码"
              aria-label="压缩包密码"
              type="password"
              bind:value={passwords[job.id]}
              onkeydown={(e) => {
                if (e.key === "Enter" && passwords[job.id]) {
                  const pw = passwords[job.id];
                  submitInstallPassword(job, pw, onInstalled);
                  delete passwords[job.id];
                }
              }}
            />
            <button
              class="accent-fill accent-text radius-pill px-3.5 h-8 text-sm font-medium cursor-pointer shrink-0"
              onclick={() => {
                const pw = passwords[job.id] ?? "";
                submitInstallPassword(job, pw, onInstalled);
                delete passwords[job.id];
              }}
            >确认</button>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => {
                delete passwords[job.id];
                dismissInstall(job);
              }}
            >关闭</button>
          {:else if job.stage === "done"}
            <span class="text-sm shrink-0">已安装到 {displayName(job.character ?? "")}</span>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => undoInstall(job, onInstalled)}
            >撤销</button>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => {
                delete passwords[job.id];
                dismissInstall(job);
              }}
            >关闭</button>
          {:else if job.stage === "error"}
            <span class="text-sm shrink-0" style="color: var(--danger)">{job.message}</span>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => retryInstall(job, onInstalled)}
            >重试</button>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => {
                delete passwords[job.id];
                dismissInstall(job);
              }}
            >关闭</button>
          {/if}
        </div>

        {#if job.stage === "pick-category"}
          <div class="flex flex-col gap-2 pl-1 -my-0.5">
            <div class="flex flex-wrap gap-1.5">
              <button
                class="glass radius-pill px-3 h-7 text-xs cursor-pointer transition-colors"
                class:accent-fill={scopeOf(job) === "character"}
                class:accent-text={scopeOf(job) === "character"}
                onclick={() => setScope(job, "character")}
              >角色</button>
              {#each fixedTypes as t (t.id)}
                <button
                  class="glass radius-pill px-3 h-7 text-xs cursor-pointer transition-colors"
                  class:accent-fill={scopeOf(job) === t.kind}
                  class:accent-text={scopeOf(job) === t.kind}
                  onclick={() => setScope(job, t.kind)}
                >{t.name}</button>
              {/each}
            </div>
            {#if scopeOf(job) === "character"}
              <div class="flex flex-wrap gap-1.5 max-h-40 overflow-y-auto">
                {#each characters as c (c.internal_name)}
                  <button
                    class="glass radius-pill px-2.5 h-6 text-[11px] cursor-pointer transition-colors"
                    class:accent-fill={charPicked(job) === c.internal_name}
                    class:accent-text={charPicked(job) === c.internal_name}
                    onclick={() => setChar(job, c.internal_name)}
                  >{c.display_name}</button>
                {/each}
              </div>
            {/if}
            <div class="flex justify-end gap-1.5 mt-0.5">
              <button
                class="accent-fill accent-text radius-pill px-3.5 h-8 text-sm font-medium cursor-pointer disabled:opacity-40"
                disabled={!(scopeOf(job) && (scopeOf(job) !== "character" || charPicked(job)))}
                onclick={() => confirmPick(job)}
              >安装</button>
            </div>
          </div>
        {/if}

        {#if job.warnings.length > 0}
          {#each job.warnings as w}
            <p class="text-xs text-secondary -mt-2 pl-1">{w}</p>
          {/each}
        {/if}
      {/each}
    </div>
  </div>
{/if}

<style>
  .spinner {
    width: 16px;
    height: 16px;
    border-radius: 9999px;
    border: 2px solid var(--glass-stroke);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
