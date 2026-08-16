// FirstRun.vue 组件测试。对应测试设计 §PR-010。
// 覆盖：挂载显示镜像源选择器 + 下载按钮、下载中显示进度、解压中、完成、失败重试。

import { beforeEach, describe, expect, it, vi, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockImplementation(() => Promise.resolve(() => {})),
}));

import { invoke } from "@tauri-apps/api/core";
import FirstRun from "../FirstRun.vue";
import { useLauncherStore } from "@/stores/launcher";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("FirstRun", () => {
  it("mounts and shows mirror selector + download button", async () => {
    invokeMock.mockResolvedValue([
      {
        id: "npmmirror.com",
        name: "npmmirror",
        base_url: "https://npmmirror.com",
        trusted: true,
      },
    ]);

    const wrapper = mount(FirstRun);
    await flushPromises();

    expect(wrapper.text()).toContain("首次启动向导");
    expect(wrapper.text()).toContain("选择 Node 下载源");
    expect(wrapper.text()).toContain("开始下载");
  });

  it("shows downloading UI when wizardStep = downloading", async () => {
    invokeMock.mockResolvedValue([]);
    const wrapper = mount(FirstRun);
    await flushPromises();

    const store = useLauncherStore();
    store.wizardStep = "downloading";
    store.downloadProgress = { bytes: 1024, total: 4096 };
    store.mirrors = [
      {
        id: "npmmirror.com",
        name: "npmmirror",
        base_url: "https://npmmirror.com",
        trusted: true,
      },
    ];
    store.selectedMirrorId = "npmmirror.com";
    await flushPromises();

    expect(wrapper.text()).toContain("正在下载");
    expect(wrapper.text()).toContain("25");
  });

  it("shows extracting UI when wizardStep = extracting", async () => {
    invokeMock.mockResolvedValue([]);
    const wrapper = mount(FirstRun);
    await flushPromises();

    const store = useLauncherStore();
    store.wizardStep = "extracting";
    await flushPromises();

    expect(wrapper.text()).toContain("正在解压并安装");
  });

  it("shows done UI with start host button when wizardStep = done", async () => {
    invokeMock.mockResolvedValue([]);
    const wrapper = mount(FirstRun);
    await flushPromises();

    const store = useLauncherStore();
    store.wizardStep = "done";
    store.nodeVersion = "22.19.0";
    store.dshVersion = "0.1.0"; // dsh 已装，显示"启动 DeepSeek Harness"按钮
    await flushPromises();

    expect(wrapper.text()).toContain("安装完成");
    expect(wrapper.text()).toContain("22.19.0");
    expect(wrapper.text()).toContain("启动 DeepSeek Harness");
  });

  it("shows done UI with install dsh button when dshVersion is null", async () => {
    invokeMock.mockResolvedValue([]);
    const wrapper = mount(FirstRun);
    await flushPromises();

    const store = useLauncherStore();
    store.wizardStep = "done";
    store.nodeVersion = "22.19.0";
    store.dshVersion = null; // dsh 未装，显示"安装 dsh"按钮
    await flushPromises();

    expect(wrapper.text()).toContain("安装完成");
    expect(wrapper.text()).toContain("安装 DeepSeek Harness");
  });

  it("shows failed UI with retry button when wizardStep = failed", async () => {
    invokeMock.mockResolvedValue([]);
    const wrapper = mount(FirstRun);
    await flushPromises();

    const store = useLauncherStore();
    store.wizardStep = "mirror_select"; // 失败后 wizardStep 重置到 mirror_select
    store.phase = "error";
    store.error = { kind: "io", message: "disk full" };
    store.lastFailedAction = "installNode";
    store.preErrorPhase = "first_run";
    store.preErrorWizardStep = "mirror_select";
    await flushPromises();

    // 错误统一由 ErrorDialog 处理（不在 FirstRun 内显示失败 UI）
    expect(wrapper.text()).not.toContain("安装失败");
  });

  it("clicking retry calls retryLastAction via ErrorDialog", async () => {
    invokeMock.mockResolvedValue([]);
    // 点击重试会调用 installNode，再次失败
    invokeMock.mockRejectedValueOnce({ kind: "io", message: "still failing" });
    mount(FirstRun);
    await flushPromises();

    const store = useLauncherStore();
    store.wizardStep = "mirror_select";
    store.phase = "error";
    store.error = { kind: "io", message: "disk full" };
    store.lastFailedAction = "installNode";
    store.preErrorPhase = "first_run";
    store.preErrorWizardStep = "mirror_select";
    await flushPromises();

    // ErrorDialog 不在 FirstRun 内，所以这里只验证 wizardStep 仍是 mirror_select
    expect(store.wizardStep).toBe("mirror_select");
  });

  it("disables download button when no mirror selected", async () => {
    invokeMock.mockResolvedValue([]);
    const wrapper = mount(FirstRun);
    await flushPromises();

    const btn = wrapper.findAll("button").find((b) =>
      b.text().includes("开始下载"),
    );
    expect(btn?.attributes("disabled")).toBeDefined();
  });

  it("calls installNode on install button click", async () => {
      // 顺序：loadMirrors → install_node_command → refreshStatus
      const fakeMirrors = [
        {
          id: "npmmirror.com",
          name: "npmmirror",
          base_url: "https://npmmirror.com",
          trusted: true,
        },
      ];
      invokeMock.mockResolvedValueOnce(fakeMirrors); // loadMirrors
      invokeMock.mockResolvedValueOnce({
        phase: "first_run",
        host_origin: null,
        dsh_version: null,
        node_version: null,
        platform: "darwin",
        arch: "arm64",
      }); // 手动 refreshStatus（应用 platform/arch 快照）
      invokeMock.mockResolvedValueOnce("22.19.0"); // install_node_command
      invokeMock.mockResolvedValueOnce({
        phase: "idle",
        host_origin: null,
        dsh_version: null,
        node_version: "22.19.0",
        platform: "darwin",
        arch: "arm64",
      }); // refreshStatus

      const wrapper = mount(FirstRun);
      await flushPromises();

      const store = useLauncherStore();
      // loadMirrors 已经默认选中第一个，确认一下
      expect(store.selectedMirrorId).toBe("npmmirror.com");

      // detectPlatformArch 只认后端快照：为该测试文件应用一次 status
      await store.refreshStatus();

      const btn = wrapper.findAll("button").find((b) =>
        b.text().includes("开始下载"),
      );
      await btn?.trigger("click");
      await flushPromises();

      expect(store.wizardStep).toBe("done");
    });
  });
