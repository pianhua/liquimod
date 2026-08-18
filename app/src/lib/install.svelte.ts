import { api } from "$lib/api";

export type InstallStage = "installing" | "needs-password" | "done" | "error";

export interface InstallJob {
  id: number;
  fileName: string;
  path: string;
  stage: InstallStage;
  character: string | null;
  modId: number | null;
  message: string | null;
  warnings: string[];
}

let nextId = 1;

export const installJobs = $state<InstallJob[]>([]);

export function enqueueInstalls(paths: string[], onInstalled: () => void): void {
  for (const path of paths) {
    const job: InstallJob = {
      id: nextId++,
      fileName: path.split(/[\\/]/).pop() ?? path,
      path,
      stage: "installing",
      character: null,
      modId: null,
      message: null,
      warnings: [],
    };
    installJobs.push(job);
    void runInstall(installJobs[installJobs.length - 1], null, onInstalled);
  }
}

async function runInstall(
  job: InstallJob,
  password: string | null,
  onInstalled: () => void,
): Promise<void> {
  job.stage = "installing";
  job.message = null;
  try {
    const result = await api.installMod(job.path, null, password);
    if (result.status === "needs_password") {
      job.stage = "needs-password";
      return;
    }
    job.stage = "done";
    job.character = result.character;
    job.modId = result.mod_id;
    job.warnings = result.warnings;
    onInstalled();
  } catch (e) {
    job.stage = "error";
    job.message = e instanceof Error ? e.message : String(e);
  }
}

export async function submitInstallPassword(
  job: InstallJob,
  password: string,
  onInstalled: () => void,
): Promise<void> {
  await runInstall(job, password, onInstalled);
}

export function retryInstall(job: InstallJob, onInstalled: () => void): void {
  void runInstall(job, null, onInstalled);
}

export async function undoInstall(
  job: InstallJob,
  onInstalled: () => void,
): Promise<void> {
  if (job.modId != null) {
    try {
      await api.uninstallMod(job.modId);
    } catch {
      // 撤销失败也移除任务条目；错误属于非阻断提示，主流程永远可用
    }
  }
  dismissInstall(job);
  onInstalled();
}

export function dismissInstall(job: InstallJob): void {
  const i = installJobs.indexOf(job);
  if (i >= 0) installJobs.splice(i, 1);
}