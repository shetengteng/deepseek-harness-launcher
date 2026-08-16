// Pinia launcher store 测试。对应测试设计 §PR-005。

import { beforeEach, describe, expect, it, vi, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

// Mock @tauri-apps/api/core 的 invoke，避免真实调用 Rust 端。
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { useLauncherStore } from "../launcher";
import type { StatusSnapshot } from "@/lib/tauri";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("useLauncherStore", () => {
  describe("initial state", () => {
    it("starts in booting phase with no origin", () => {
      const store = useLauncherStore();
      expect(store.phase).toBe("booting");
      expect(store.origin).toBeNull();
      expect(store.error).toBeNull();
      expect(store.dshVersion).toBeNull();
      expect(store.nodeVersion).toBeNull();
      expect(store.starting).toBe(false);
      expect(store.stopping).toBe(false);
    });
  });

  describe("refreshStatus", () => {
    it("transitions to first_run when snapshot says first_run", async () => {
      invokeMock.mockResolvedValueOnce({
        phase: "first_run",
        host_origin: null,
        dsh_version: null,
        node_version: null,
      } satisfies StatusSnapshot);

      const store = useLauncherStore();
      await store.refreshStatus();

      expect(store.phase).toBe("first_run");
      expect(store.origin).toBeNull();
    });

    it("transitions to idle with versions when snapshot has dsh/node", async () => {
      invokeMock.mockResolvedValueOnce({
        phase: "idle",
        host_origin: null,
        dsh_version: "0.1.0",
        node_version: "20.18.0",
      } satisfies StatusSnapshot);

      const store = useLauncherStore();
      await store.refreshStatus();

      expect(store.phase).toBe("idle");
      expect(store.dshVersion).toBe("0.1.0");
      expect(store.nodeVersion).toBe("20.18.0");
    });

    it("transitions to error on failure with LauncherError payload", async () => {
      const payload = {
        kind: "host",
        message: "desktop Host cannot start after shutdown",
      };
      invokeMock.mockRejectedValueOnce(payload);

      const store = useLauncherStore();
      await store.refreshStatus();

      expect(store.phase).toBe("error");
      expect(store.error).toEqual(payload);
    });

    it("wraps non-payload errors into io error", async () => {
      invokeMock.mockRejectedValueOnce(new Error("network down"));

      const store = useLauncherStore();
      await store.refreshStatus();

      expect(store.phase).toBe("error");
      expect(store.error?.kind).toBe("io");
      expect(store.error?.message).toBe("network down");
    });
  });

  describe("startHost", () => {
    it("transitions to ready with origin on success", async () => {
      invokeMock.mockResolvedValueOnce("http://127.0.0.1:51329");

      const store = useLauncherStore();
      await store.startHost();

      expect(store.phase).toBe("ready");
      expect(store.origin).toBe("http://127.0.0.1:51329");
      expect(store.error).toBeNull();
      expect(store.starting).toBe(false);
    });

    it("guards against concurrent calls", async () => {
      let resolve: ((v: string) => void) | undefined;
      invokeMock.mockImplementationOnce(
        () => new Promise<string>((r) => (resolve = r)),
      );

      const store = useLauncherStore();
      const p1 = store.startHost();
      const p2 = store.startHost();

      expect(store.starting).toBe(true);
      resolve?.("http://127.0.0.1:1");
      await Promise.all([p1, p2]);

      expect(invokeMock).toHaveBeenCalledTimes(1);
    });

    it("transitions to error on failure", async () => {
      invokeMock.mockRejectedValueOnce({
        kind: "host",
        message: "spawn failed",
      });

      const store = useLauncherStore();
      await store.startHost();

      expect(store.phase).toBe("error");
      expect(store.error?.kind).toBe("host");
      expect(store.starting).toBe(false);
    });
  });

  describe("shutdownHost", () => {
    it("transitions ready → idle on success", async () => {
      invokeMock.mockResolvedValueOnce(undefined);

      const store = useLauncherStore();
      store.phase = "ready";
      store.origin = "http://127.0.0.1:1";

      await store.shutdownHost();

      expect(store.phase).toBe("idle");
      expect(store.origin).toBeNull();
    });

    it("leaves non-ready phase unchanged", async () => {
      invokeMock.mockResolvedValueOnce(undefined);

      const store = useLauncherStore();
      store.phase = "first_run";

      await store.shutdownHost();

      expect(store.phase).toBe("first_run");
    });

    it("guards against concurrent calls", async () => {
      let resolve: (() => void) | undefined;
      invokeMock.mockImplementationOnce(
        () => new Promise<void>((r) => (resolve = r)),
      );

      const store = useLauncherStore();
      const p1 = store.shutdownHost();
      const p2 = store.shutdownHost();

      expect(store.stopping).toBe(true);
      resolve?.();
      await Promise.all([p1, p2]);

      expect(invokeMock).toHaveBeenCalledTimes(1);
    });
  });

  describe("resetError", () => {
    it("clears error and restores preErrorPhase when kind is not node/dsh not_installed", () => {
      const store = useLauncherStore();
      // 模拟 idle 视图启动 Host 失败：phase=error, preErrorPhase=idle
      store.phase = "idle";
      store.preErrorPhase = "idle";
      store.lastFailedAction = "startHost";
      store.error = { kind: "io", message: "boom" };
      store.phase = "error";

      store.resetError();

      // io error → 恢复到错误前 phase（idle）
      expect(store.phase).toBe("idle");
      expect(store.error).toBeNull();
    });

    it("transitions to first_run + wizardStep=done when kind is dsh_not_installed and nodeVersion set", () => {
      const store = useLauncherStore();
      store.phase = "idle";
      store.nodeVersion = "22.19.0";
      store.preErrorPhase = "idle";
      store.lastFailedAction = "startHost";
      store.error = { kind: "dsh_not_installed", message: "dsh not installed" };
      store.phase = "error";

      store.resetError();

      // dsh_not_installed → first_run + wizardStep=done（因为 nodeVersion 已设）
      expect(store.phase).toBe("first_run");
      expect(store.wizardStep).toBe("done");
      expect(store.error).toBeNull();
    });

    it("is a no-op when error is null", () => {
      const store = useLauncherStore();
      store.phase = "ready";

      store.resetError();

      expect(store.phase).toBe("ready");
    });
  });

  // ─── PR-011 首启向导测试 ───

  describe("wizard initial state", () => {
    it("starts at mirror_select step", () => {
      const store = useLauncherStore();
      expect(store.wizardStep).toBe("mirror_select");
      expect(store.mirrors).toEqual([]);
      expect(store.selectedMirrorId).toBeNull();
      expect(store.customMirrorUrl).toBe("");
      expect(store.customMirrorValidation).toBeNull();
      expect(store.downloadProgress).toEqual({ bytes: 0, total: null });
      expect(store.extractProgress).toBe(0);
      expect(store.installing).toBe(false);
    });
  });

  describe("loadMirrors", () => {
    it("loads builtin mirrors and selects first by default", async () => {
      const fakeMirrors = [
        {
          id: "npmmirror.com",
          name: "npmmirror",
          base_url: "https://registry.npmmirror.com/mirrors/node",
          trusted: true,
        },
        {
          id: "nodejs.org",
          name: "Node.js 官方",
          base_url: "https://nodejs.org/dist",
          trusted: true,
        },
      ];
      invokeMock.mockResolvedValueOnce(fakeMirrors);

      const store = useLauncherStore();
      await store.loadMirrors();

      expect(store.mirrors).toEqual(fakeMirrors);
      expect(store.selectedMirrorId).toBe("npmmirror.com");
    });

    it("keeps selectedMirrorId if already set", async () => {
      invokeMock.mockResolvedValueOnce([
        {
          id: "npmmirror.com",
          name: "npmmirror",
          base_url: "https://npmmirror.com",
          trusted: true,
        },
      ]);

      const store = useLauncherStore();
      store.selectedMirrorId = "nodejs.org";
      await store.loadMirrors();

      expect(store.selectedMirrorId).toBe("nodejs.org");
    });
  });

  describe("selectMirror", () => {
    it("updates selectedMirrorId", () => {
      const store = useLauncherStore();
      store.selectMirror("nodejs.org");
      expect(store.selectedMirrorId).toBe("nodejs.org");
    });
  });

  describe("validateCustomMirror", () => {
    it("stores MirrorInfo on success", async () => {
      const fakeMirror = {
        id: "https://my-mirror.com/node",
        name: "自定义",
        base_url: "https://my-mirror.com/node",
        trusted: false,
      };
      invokeMock.mockResolvedValueOnce(fakeMirror);

      const store = useLauncherStore();
      await store.validateCustomMirror("https://my-mirror.com/node");

      expect(store.customMirrorValidation).toEqual(fakeMirror);
    });

    it("stores error string on failure", async () => {
      invokeMock.mockRejectedValueOnce({ kind: "mirror", message: "not https" });

      const store = useLauncherStore();
      await store.validateCustomMirror("http://insecure.com");

      expect(store.customMirrorValidation).toBe("not https");
    });

    it("clears validation when url is empty", async () => {
      const store = useLauncherStore();
      store.customMirrorValidation = "old error";
      await store.validateCustomMirror("");
      expect(store.customMirrorValidation).toBeNull();
    });
  });

  describe("selectedMirror (computed)", () => {
    it("returns null when no selection", () => {
      const store = useLauncherStore();
      expect(store.selectedMirror).toBeNull();
    });

    it("returns builtin mirror by id", () => {
      const store = useLauncherStore();
      store.mirrors = [
        {
          id: "npmmirror.com",
          name: "npmmirror",
          base_url: "https://npmmirror.com",
          trusted: true,
        },
      ];
      store.selectedMirrorId = "npmmirror.com";
      expect(store.selectedMirror?.id).toBe("npmmirror.com");
    });

    it("returns custom mirror when validation passes", () => {
      const store = useLauncherStore();
      store.customMirrorUrl = "https://my.com/node";
      store.customMirrorValidation = {
        id: "https://my.com/node",
        name: "自定义",
        base_url: "https://my.com/node",
        trusted: false,
      };
      store.selectedMirrorId = "https://my.com/node";
      expect(store.selectedMirror?.base_url).toBe("https://my.com/node");
    });
  });

  describe("downloadPercent (computed)", () => {
    it("returns 0 when total is null", () => {
      const store = useLauncherStore();
      store.downloadProgress = { bytes: 100, total: null };
      expect(store.downloadPercent).toBe(0);
    });

    it("returns 0 when total is 0", () => {
      const store = useLauncherStore();
      store.downloadProgress = { bytes: 0, total: 0 };
      expect(store.downloadPercent).toBe(0);
    });

    it("returns rounded percentage", () => {
      const store = useLauncherStore();
      store.downloadProgress = { bytes: 50, total: 200 };
      expect(store.downloadPercent).toBe(25);
    });

    it("caps at 100", () => {
      const store = useLauncherStore();
      store.downloadProgress = { bytes: 300, total: 200 };
      expect(store.downloadPercent).toBe(100);
    });
  });

  describe("applyProgressEvent", () => {
    it("updates downloadProgress for download stage", () => {
      const store = useLauncherStore();
      store.applyProgressEvent({ stage: "download", bytes: 1024, total: 4096 });
      expect(store.downloadProgress).toEqual({ bytes: 1024, total: 4096 });
    });

    it("transitions to extracting on extract start (total=null)", () => {
      const store = useLauncherStore();
      store.applyProgressEvent({ stage: "extract", bytes: 0, total: null });
      expect(store.wizardStep).toBe("extracting");
      expect(store.extractProgress).toBe(0.5);
    });

    it("marks extract complete when total=0", () => {
      const store = useLauncherStore();
      store.applyProgressEvent({ stage: "extract", bytes: 0, total: 0 });
      expect(store.extractProgress).toBe(1);
    });
  });

  describe("installNode", () => {
    it("transitions through downloading to done on success", async () => {
      invokeMock.mockResolvedValueOnce("22.19.0"); // install_node_command
      // refreshStatus after install
      invokeMock.mockResolvedValueOnce({
        phase: "idle",
        host_origin: null,
        dsh_version: null,
        node_version: "22.19.0",
      });

      const store = useLauncherStore();
      store.mirrors = [
        {
          id: "npmmirror.com",
          name: "npmmirror",
          base_url: "https://npmmirror.com",
          trusted: true,
        },
      ];
      store.selectedMirrorId = "npmmirror.com";

      await store.installNode();

      expect(store.wizardStep).toBe("done");
      expect(store.installing).toBe(false);
      expect(store.nodeVersion).toBe("22.19.0");
    });

    it("resets to mirror_select and sets error on install failure", async () => {
      invokeMock.mockRejectedValueOnce({ kind: "io", message: "disk full" });

      const store = useLauncherStore();
      store.mirrors = [
        {
          id: "npmmirror.com",
          name: "npmmirror",
          base_url: "https://npmmirror.com",
          trusted: true,
        },
      ];
      store.selectedMirrorId = "npmmirror.com";

      await store.installNode();

      // 失败后 wizardStep 重置到 mirror_select（page-flow-analysis.md §3.4）
      expect(store.wizardStep).toBe("mirror_select");
      expect(store.phase).toBe("error");
      expect(store.error?.message).toBe("disk full");
      expect(store.installing).toBe(false);
      expect(store.lastFailedAction).toBe("installNode");
    });

    it("sets error when no mirror selected", async () => {
      const store = useLauncherStore();
      await store.installNode();
      expect(store.phase).toBe("error");
      expect(store.error?.message).toContain("未选择镜像源");
    });

    it("guards against concurrent calls", async () => {
      let resolve: ((v: string) => void) | undefined;
      invokeMock.mockImplementationOnce(
        () => new Promise<string>((r) => (resolve = r)),
      );

      const store = useLauncherStore();
      store.mirrors = [
        {
          id: "npmmirror.com",
          name: "npmmirror",
          base_url: "https://npmmirror.com",
          trusted: true,
        },
      ];
      store.selectedMirrorId = "npmmirror.com";

      const p1 = store.installNode();
      const p2 = store.installNode();
      resolve?.("22.19.0");
      // 第二次 install 会立即 throw（已 installing），但被 fail 捕获
      await Promise.all([p1, p2]);

      // 只调一次 install_node_command
      const installCalls = invokeMock.mock.calls.filter(
        (c) => c[0] === "install_node_command",
      );
      expect(installCalls.length).toBe(1);
    });
  });

  describe("resetWizard", () => {
    it("resets to mirror_select and clears progress", () => {
      const store = useLauncherStore();
      store.wizardStep = "done";
      store.downloadProgress = { bytes: 100, total: 200 };
      store.extractProgress = 0.5;
      store.error = { kind: "io", message: "boom" };

      store.resetWizard();

      expect(store.wizardStep).toBe("mirror_select");
      expect(store.downloadProgress).toEqual({ bytes: 0, total: null });
      expect(store.extractProgress).toBe(0);
      expect(store.error).toBeNull();
    });
  });

  describe("autoPickMirror", () => {
    it("probes and selects first available mirror", async () => {
      const picked = {
        id: "nodejs.org",
        name: "Node.js 官方",
        base_url: "https://nodejs.org/dist",
        trusted: true,
      };
      invokeMock.mockResolvedValueOnce(picked);

      const store = useLauncherStore();
      store.mirrors = [
        {
          id: "npmmirror.com",
          name: "npmmirror",
          base_url: "https://npmmirror.com",
          trusted: true,
        },
      ];
      await store.autoPickMirror();

      expect(store.selectedMirrorId).toBe("nodejs.org");
      // 选中后添加到 mirrors 列表（如果不在）
      expect(store.mirrors.some((m) => m.id === "nodejs.org")).toBe(true);
      expect(store.wizardStep).toBe("mirror_select");
    });

    it("falls back to mirror_select on failure", async () => {
      invokeMock.mockRejectedValueOnce({ kind: "mirror", message: "all down" });

      const store = useLauncherStore();
      await store.autoPickMirror();

      expect(store.wizardStep).toBe("mirror_select");
      expect(store.phase).toBe("error");
    });
  });
});
