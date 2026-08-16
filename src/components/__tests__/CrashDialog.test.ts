// CrashDialog.vue 组件测试。对应设计 §5.5 / PR-017。

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import CrashDialog from "../CrashDialog.vue";
import type { CrashLimitPayload } from "@/lib/tauri";

const dialogStubs = {
  // reka-ui Dialog 在 jsdom 下 teleport 行为复杂，stub 掉
  Dialog: {
    template: '<div v-if="open"><slot /></div>',
    props: ["open"],
  },
  DialogContent: { template: "<div><slot /></div>" },
  DialogHeader: { template: "<div><slot /></div>" },
  DialogTitle: { template: "<h2><slot /></h2>" },
  DialogDescription: { template: "<p><slot /></p>" },
  DialogFooter: { template: "<div><slot /></div>" },
};

const payload: CrashLimitPayload = {
  crash_counter: 3,
  retry_limit: 3,
  exit_code: 1,
  exit_signal: null,
  known_good: "0.2.0",
};

function mountDialog(props: Partial<{ crash: CrashLimitPayload | null; recovering: boolean }> = {}) {
  return mount(CrashDialog, {
    props: {
      crash: payload,
      recovering: false,
      ...props,
    },
    global: { stubs: dialogStubs },
  });
}

describe("CrashDialog", () => {
  it("does not render content when crash is null", () => {
    const wrapper = mountDialog({ crash: null });
    expect(wrapper.text()).not.toContain("反复崩溃");
  });

  it("renders crash counter, retry limit and exit detail", () => {
    const wrapper = mountDialog();
    const text = wrapper.text();
    expect(text).toContain("反复崩溃");
    expect(text).toContain("3");
    expect(text).toContain("退出码 1");
  });

  it("hides rollback button when known_good is null", () => {
    const wrapper = mountDialog({
      crash: { ...payload, known_good: null },
    });
    const buttons = wrapper.findAll("button");
    expect(buttons.some((b) => b.text().includes("回滚"))).toBe(false);
    // 忽略 + 重试启动 仍然存在
    expect(buttons.some((b) => b.text() === "忽略")).toBe(true);
    expect(buttons.some((b) => b.text().includes("重试启动"))).toBe(true);
  });

  it("shows rollback button with known_good version", () => {
    const wrapper = mountDialog();
    const rollback = wrapper.findAll("button").find((b) => b.text().includes("回滚"));
    expect(rollback).toBeDefined();
    expect(rollback!.text()).toContain("0.2.0");
  });

  it("disables all buttons while recovering", async () => {
    const wrapper = mountDialog({ recovering: true });
    const buttons = wrapper.findAll("button");
    // recovering 时所有按钮禁用，文案切换
    expect(buttons.every((b) => b.attributes("disabled") !== undefined)).toBe(true);
    expect(wrapper.text()).toContain("重启中…");
  });

  it("emits retry when retry button clicked", async () => {
    const wrapper = mountDialog();
    const retry = wrapper.findAll("button").find((b) => b.text().includes("重试启动"));
    await retry!.trigger("click");
    expect(wrapper.emitted("retry")).toBeTruthy();
  });

  it("emits rollback when rollback button clicked", async () => {
    const wrapper = mountDialog();
    const rollback = wrapper.findAll("button").find((b) => b.text().includes("回滚"));
    await rollback!.trigger("click");
    expect(wrapper.emitted("rollback")).toBeTruthy();
  });

  it("emits dismiss when ignore button clicked", async () => {
    const wrapper = mountDialog();
    const ignore = wrapper.findAll("button").find((b) => b.text() === "忽略");
    await ignore!.trigger("click");
    expect(wrapper.emitted("dismiss")).toBeTruthy();
  });

  it("omits exit detail when code and signal are null", () => {
    const wrapper = mountDialog({
      crash: { ...payload, exit_code: null, exit_signal: null },
    });
    expect(wrapper.text()).not.toContain("退出码");
    expect(wrapper.text()).not.toContain("信号");
  });

  it("renders exit signal when present", () => {
    const wrapper = mountDialog({
      crash: { ...payload, exit_code: null, exit_signal: 9 },
    });
    expect(wrapper.text()).toContain("信号 9");
  });
});
