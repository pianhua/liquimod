import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("$lib/api", () => ({
  api: {
    installMod: vi.fn(),
    uninstallMod: vi.fn(),
  },
}));

import { api } from "$lib/api";
import {
  dismissInstall,
  enqueueInstalls,
  installJobs,
  submitInstallPassword,
  undoInstall,
} from "./install.svelte";

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("install queue", () => {
  beforeEach(() => {
    installJobs.length = 0;
    vi.clearAllMocks();
  });

  it("installs successfully and calls back", async () => {
    vi.mocked(api.installMod).mockResolvedValue({
      status: "installed",
      mod_id: 7,
      name: "Cool",
      character: "Firefly",
      warnings: [],
    });
    const onInstalled = vi.fn();

    enqueueInstalls(["C:/dl/Cool.zip"], onInstalled);
    expect(installJobs).toHaveLength(1);
    expect(installJobs[0].stage).toBe("installing");
    await flush(); await flush();

    expect(installJobs[0].stage).toBe("done");
    expect(installJobs[0].character).toBe("Firefly");
    expect(installJobs[0].modId).toBe(7);
    expect(onInstalled).toHaveBeenCalledOnce();
  });

  it("password flow: needs-password then submit", async () => {
    vi.mocked(api.installMod)
      .mockResolvedValueOnce({ status: "needs_password" })
      .mockResolvedValueOnce({
        status: "installed",
        mod_id: 8,
        name: "Locked",
        character: "Kafka",
        warnings: [],
      });

    enqueueInstalls(["C:/dl/Locked.zip"], vi.fn());
    await flush(); await flush();
    expect(installJobs[0].stage).toBe("needs-password");
    expect(api.installMod).toHaveBeenCalledWith("C:/dl/Locked.zip", null, null);

    await submitInstallPassword(installJobs[0], "pw", vi.fn());
    await flush();
    expect(api.installMod).toHaveBeenLastCalledWith("C:/dl/Locked.zip", null, "pw");
    expect(installJobs[0].stage).toBe("done");
  });

  it("error stage keeps human message and retry works", async () => {
    vi.mocked(api.installMod)
      .mockRejectedValueOnce(new Error("已存在同名 Mod：Dup"))
      .mockResolvedValueOnce({
        status: "installed",
        mod_id: 9,
        name: "Dup",
        character: "Others",
        warnings: [],
      });

    enqueueInstalls(["C:/dl/Dup.zip"], vi.fn());
    await flush(); await flush();
    expect(installJobs[0].stage).toBe("error");
    expect(installJobs[0].message).toContain("已存在同名 Mod");

    const { retryInstall } = await import("./install.svelte");
    retryInstall(installJobs[0], vi.fn());
    await flush(); await flush();
    expect(installJobs[0].stage).toBe("done");
  });

  it("undo uninstalls and removes the job", async () => {
    vi.mocked(api.installMod).mockResolvedValue({
      status: "installed",
      mod_id: 11,
      name: "X",
      character: "Bailu",
      warnings: [],
    });
    vi.mocked(api.uninstallMod).mockResolvedValue(undefined);
    const onInstalled = vi.fn();

    enqueueInstalls(["C:/dl/X.zip"], onInstalled);
    await flush(); await flush();

    await undoInstall(installJobs[0], onInstalled);
    expect(api.uninstallMod).toHaveBeenCalledWith(11);
    expect(installJobs).toHaveLength(0);
    expect(onInstalled).toHaveBeenCalledTimes(2);
  });

  it("dismiss removes without side effects", () => {
    installJobs.push({
      id: 999,
      fileName: "a.zip",
      path: "C:/a.zip",
      stage: "done",
      character: "Bailu",
      modId: 1,
      message: null,
      warnings: [],
    });
    dismissInstall(installJobs[0]);
    expect(installJobs).toHaveLength(0);
  });
});