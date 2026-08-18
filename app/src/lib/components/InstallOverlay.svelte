<script lang="ts">
  import {
    dismissInstall,
    retryInstall,
    submitInstallPassword,
    undoInstall,
    type InstallJob,
  } from "$lib/install.svelte";
  import type { CharacterSummary } from "$lib/api";

  let {
    jobs,
    characters,
    onInstalled,
  }: {
    jobs: InstallJob[];
    characters: CharacterSummary[];
    onInstalled: () => void;
  } = $props();

  let passwords = $state<Record<number, string>>({});

  function displayName(internal: string): string {
    return characters.find((c) => c.internal_name === internal)?.display_name ?? internal;
  }
</script>

{#if jobs.length > 0}
  <div class="install-overlay fixed bottom-6 inset-x-0 z-50 flex justify-center pointer-events-none" aria-live="polite">
    <div class="glass radius-panel pointer-events-auto w-[420px] max-w-[90vw] px-5 py-4 flex flex-col gap-3 max-h-[70vh] overflow-y-auto"
      style="box-shadow: var(--shadow-lift)">
      {#each jobs as job (job.id)}
        <div class="flex items-center gap-3 min-h-9">
          {#if job.stage === "installing"}
            <span class="spinner shrink-0"></span>
          {/if}
          <span class="text-sm font-medium truncate flex-1 min-w-0">{job.fileName}</span>

          {#if job.stage === "installing"}
            <span class="text-sm text-secondary shrink-0">正在安装…</span>
          {:else if job.stage === "needs-password"}
            <input
              class="glass radius-pill px-3 h-8 text-sm w-32 outline-none bg-transparent text-white"
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