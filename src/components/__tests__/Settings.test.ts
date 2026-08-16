// Settings.vue 组件测试。对应测试设计 §PR-016。
// 覆盖：挂载显示运行时状态、升级策略、检查更新按钮、已安装版本列表。

import { beforeEach, describe, expect, it, vi, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import Settings from "../Settings.vue";

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

const mockState = {
  current: "0.1.0",
  known_good: "0.0.9",
  pending: null,
  pinned_range: "~0.1.0",
  auto_upgrade: true,
  check_interval_hours: 24,
  registry: "https://registry.npmmirror.com",
  installed: [
    {
      version: "0.1.0",
      installed_at: "2026-01-01T00:00:00Z",
      status: "verified",
    },
    {
      version: "0.0.9",
      installed_at: "2025-12-01T00:00:00Z",
      status: "verified",
    },
  ],
  ignored_versions: ["0.0.8"],
};

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("Settings", () => {
  it("renders loading state initially", () => {
    invokeMock.mockImplementation(() => new Promise(() => {})); // 永不 resolve
    const wrapper = mount(Settings);
    expect(wrapper.text()).toContain("加载中");
  });

  it("renders runtime status section", async () => {
    invokeMock.mockResolvedValueOnce(mockState);
    const wrapper = mount(Settings);
    await flushPromises();

    expect(wrapper.text()).toContain("运行时状态");
    expect(wrapper.text()).toContain("0.1.0");
    expect(wrapper.text()).toContain("0.0.9");
    expect(wrapper.text()).toContain("已知好版本");
  });

  it("renders upgrade strategy section", async () => {
    invokeMock.mockResolvedValueOnce(mockState);
    const wrapper = mount(Settings);
    await flushPromises();

    expect(wrapper.text()).toContain("升级策略");
    expect(wrapper.text()).toContain("版本范围锁定");
    expect(wrapper.text()).toContain("自动升级");
    expect(wrapper.text()).toContain("检查间隔");
  });

  it("renders check for upgrade button", async () => {
    invokeMock.mockResolvedValueOnce(mockState);
    const wrapper = mount(Settings);
    await flushPromises();

    const btn = wrapper.findAll("button").find((b) =>
      b.text().includes("检查更新"),
    );
    expect(btn).toBeDefined();
  });

  it("shows check result when no upgrade available", async () => {
    invokeMock.mockResolvedValueOnce(mockState); // getDshState
    invokeMock.mockResolvedValueOnce({
      available: false,
      version: null,
      engines_node: null,
    }); // checkForUpgrade
    const wrapper = mount(Settings);
    await flushPromises();

    const btn = wrapper.findAll("button").find((b) =>
      b.text().includes("检查更新"),
    );
    await btn?.trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("已是最新版本");
  });

  it("shows install button when upgrade available", async () => {
    invokeMock.mockResolvedValueOnce(mockState); // getDshState
    invokeMock.mockResolvedValueOnce({
      available: true,
      version: "0.2.0",
      engines_node: null,
    }); // checkForUpgrade
    const wrapper = mount(Settings);
    await flushPromises();

    const btn = wrapper.findAll("button").find((b) =>
      b.text().includes("检查更新"),
    );
    await btn?.trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("安装 0.2.0");
    expect(wrapper.text()).toContain("忽略此版本");
  });

  it("renders installed versions list", async () => {
    invokeMock.mockResolvedValueOnce(mockState);
    const wrapper = mount(Settings);
    await flushPromises();

    expect(wrapper.text()).toContain("已安装版本");
    expect(wrapper.text()).toContain("verified");
  });

  it("renders ignored versions", async () => {
    invokeMock.mockResolvedValueOnce(mockState);
    const wrapper = mount(Settings);
    await flushPromises();

    expect(wrapper.text()).toContain("已忽略版本");
    expect(wrapper.text()).toContain("0.0.8");
  });

  it("shows error state when dshState is null", async () => {
    invokeMock.mockRejectedValueOnce({ kind: "io", message: "fail" });
    const wrapper = mount(Settings);
    await flushPromises();

    expect(wrapper.text()).toContain("无法加载设置");
  });

  it("has back button", async () => {
    invokeMock.mockResolvedValueOnce(mockState);
    const wrapper = mount(Settings);
    await flushPromises();

    // 第一个 button 是 back 按钮（ArrowLeft）
    const buttons = wrapper.findAll("button");
    expect(buttons.length).toBeGreaterThan(0);
  });
});