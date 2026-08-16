// MainView.vue 组件测试。对应测试设计 §PR-005 + §PR-010（PR-011 首启向导）。

import { beforeEach, describe, expect, it, vi, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock Tauri event listen（FirstRun.vue 在 onMounted 调用 listen）
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockImplementation(() => Promise.resolve(() => {})),
}));

import { invoke } from "@tauri-apps/api/core";
import MainView from "../MainView.vue";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("MainView", () => {
  it("renders loading spinner in booting phase", () => {
    invokeMock.mockImplementation(() => new Promise(() => {})); // 永不 resolve
    const wrapper = mount(MainView);
    expect(wrapper.text()).toContain("正在初始化");
  });

  it("renders first_run wizard after fetchStatus returns first_run", async () => {
    // FirstRun.vue 挂载时会调 list_mirrors + refreshStatus
    invokeMock.mockResolvedValue([]); // 默认所有 invoke 返回空数组，list_mirrors 兼容
    invokeMock.mockResolvedValueOnce({
      phase: "first_run",
      host_origin: null,
      dsh_version: null,
      node_version: null,
    });
    const wrapper = mount(MainView);
    await flushPromises();

    expect(wrapper.text()).toContain("首次启动向导");
  });

  it("renders idle card with versions", async () => {
    invokeMock.mockResolvedValueOnce({
      phase: "idle",
      host_origin: null,
      dsh_version: "0.1.0",
      node_version: "20.18.0",
    });
    const wrapper = mount(MainView);
    await flushPromises();

    expect(wrapper.text()).toContain("0.1.0");
    expect(wrapper.text()).toContain("20.18.0");
    expect(wrapper.text()).toContain("启动 DeepSeek Harness");
  });

  it("renders iframe when ready with origin", async () => {
    invokeMock.mockResolvedValueOnce({
      phase: "idle",
      host_origin: null,
      dsh_version: "0.1.0",
      node_version: "20.18.0",
    });
    invokeMock.mockResolvedValueOnce("http://127.0.0.1:51329");

    const wrapper = mount(MainView);
    await flushPromises();
    // 点击"启动 DeepSeek Harness"按钮（不是 Settings 图标按钮）
    const startBtn = wrapper.findAll("button").find((b) =>
      b.text().includes("启动 DeepSeek Harness"),
    );
    await startBtn?.trigger("click");
    await flushPromises();

    const iframe = wrapper.find("iframe");
    expect(iframe.exists()).toBe(true);
    expect(iframe.attributes("src")).toBe("http://127.0.0.1:51329");
  });

  it("calls refreshStatus on mount", async () => {
    invokeMock.mockResolvedValueOnce({
      phase: "idle",
      host_origin: null,
      dsh_version: null,
      node_version: null,
    });
    mount(MainView);
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith("launcher_status", undefined);
  });

  it("renders ErrorDialog when phase is error", async () => {
    invokeMock.mockRejectedValueOnce({ kind: "io", message: "boom" });
    const wrapper = mount(MainView);
    await flushPromises();

    // 错误时 displayPhase=idle（preErrorPhase=booting 时回退到 idle）
    expect(wrapper.text()).not.toContain("正在初始化");
    // idle 视图显示
    expect(wrapper.text()).toContain("DeepSeek Harness");
    // ErrorDialog 组件存在（reka-ui Dialog 在 jsdom 下不渲染内容，只验证组件挂载）
    expect(wrapper.findComponent({ name: "ErrorDialog" }).exists()).toBe(true);
  });
});
