import { api } from "$lib/api";

export type InstallStage = "installing" | "needs-password" | "done" | "error" | "pick-category";

/** 安装上下文：目标角色名 或 固定分类 kind（npc/lightcone/portrait/scene/other）。 */
export type InstallTarget = string | null;

export interface InstallJob {
  id: number;
  fileName: string;
  path: string;
  stage: InstallStage;
  /** 安装目标：角色内部名 或 固定分类 kind；安装后为后端返回的 character。 */
  character: string | null;
  modId: number | null;
  message: string | null;
  warnings: string[];
  busy: boolean;
}

let nextId = 1;

export const installJobs = $state<InstallJob[]>([]);

/** target 非空直接装；null 则进「选分类」阶段，选完再装。 */
export function enqueueInstalls(
  paths: string[],
  target: InstallTarget,
  onInstalled: () => void,
): void {
  for (const path of paths) {
    const job: InstallJob = {
      id: nextId++,
      fileName: path.split(/[\\/]/).pop() ?? path,
      path,
      stage: target ? "installing" : "pick-category",
      character: target,
      modId: null,
      message: null,
      warnings: [],
      busy: false,
    };
    installJobs.push(job);
    if (target) void runInstall(installJobs[installJobs.length - 1], null, onInstalled);
  }
}

/** 分类弹窗选定后开始安装：target 为角色内部名或固定分类 kind。 */
export function startInstallWithCategory(
  job: InstallJob,
  target: string,
  onInstalled: () => void,
): void {
  job.character = target;
  void runInstall(job, null, onInstalled);
}

async function runInstall(
  job: InstallJob,
  password: string | null,
  onInstalled: () => void,
): Promise<void> {
  if (job.busy) return;
  job.busy = true;
  job.stage = "installing";
  job.message = null;
  try {
    const result = await api.installMod(job.path, job.character, password);
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
  } finally {
    job.busy = false;
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
