// upgrade store 测试。对应测试设计 §PR-016。
// 覆盖：初始状态、检查升级、安装升级、对话框控制。

import { beforeEach, describe, expect, it, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { useUpgradeStore } from "@/stores/upgrade";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
});

describe("upgrade store", () => {
  it("initial state is idle", () => {
    const store = useUpgradeStore();
    expect(store.available).toBe(false);
    expect(store.version).toBeNull();
    expect(store.checking).toBe(false);
    expect(store.upgrading).toBe(false);
    expect(store.showDialog).toBe(false);
    expect(store.error).toBeNull();
  });

  it("check sets available when upgrade exists", async () => {
    invokeMock.mockResolvedValueOnce({
      available: true,
      version: "0.2.0",
      engines_node: null,
    });

    const store = useUpgradeStore();
    const result = await store.check();

    expect(result.available).toBe(true);
    expect(result.version).toBe("0.2.0");
    expect(store.available).toBe(true);
    expect(store.version).toBe("0.2.0");
    expect(store.checking).toBe(false);
  });

  it("check sets available=false when no upgrade", async () => {
    invokeMock.mockResolvedValueOnce({
      available: false,
      version: null,
      engines_node: null,
    });

    const store = useUpgradeStore();
    const result = await store.check();

    expect(result.available).toBe(false);
    expect(store.available).toBe(false);
    expect(store.version).toBeNull();
  });

  it("check handles errors gracefully", async () => {
    invokeMock.mockRejectedValueOnce({ kind: "dsh_registry", message: "network error" });

    const store = useUpgradeStore();
    const result = await store.check();

    expect(result.available).toBe(false);
    expect(store.available).toBe(false);
    expect(store.error).toBe("network error");
    expect(store.checking).toBe(false);
  });

  it("check prevents concurrent calls", async () => {
    // 第一次调用 pending 时，第二次应被跳过
    let resolveFirst: (value: unknown) => void;
    const firstCall = new Promise((resolve) => { resolveFirst = resolve; });
    invokeMock.mockReturnValueOnce(firstCall);

    const store = useUpgradeStore();
    const p1 = store.check(); // 开始第一次检查
    const p2 = store.check(); // 第二次应被跳过

    // 第二次调用应直接返回 no-upgrade
    const result2 = await p2;
    expect(result2!.available).toBe(false);

    // 完成第一次调用
    resolveFirst!({ available: true, version: "0.2.0", engines_node: null });
    const result1 = await p1;
    expect(result1!.available).toBe(true);
  });

  it("prepare installs and shows dialog", async () => {
    invokeMock.mockResolvedValueOnce("0.2.0");

    const store = useUpgradeStore();
    const version = await store.prepare();

    expect(version).toBe("0.2.0");
    expect(store.version).toBe("0.2.0");
    expect(store.showDialog).toBe(true);
    expect(store.upgrading).toBe(false);
  });

  it("prepare handles errors", async () => {
    invokeMock.mockRejectedValueOnce({ kind: "dsh_install", message: "install failed" });

    const store = useUpgradeStore();
    const version = await store.prepare();

    expect(version).toBeNull();
    expect(store.error).toBe("install failed");
    expect(store.showDialog).toBe(false);
  });

  it("dismissDialog closes dialog", () => {
    const store = useUpgradeStore();
    store.showDialog = true;
    store.dismissDialog();
    expect(store.showDialog).toBe(false);
  });

  it("reset clears all state", () => {
    const store = useUpgradeStore();
    store.available = true;
    store.version = "0.2.0";
    store.checking = true;
    store.upgrading = true;
    store.showDialog = true;
    store.error = "err";

    store.reset();

    expect(store.available).toBe(false);
    expect(store.version).toBeNull();
    expect(store.checking).toBe(false);
    expect(store.upgrading).toBe(false);
    expect(store.showDialog).toBe(false);
    expect(store.error).toBeNull();
  });
});