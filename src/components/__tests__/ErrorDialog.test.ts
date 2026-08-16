// ErrorDialog.vue 组件测试。对应测试设计 §PR-005。

import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ErrorDialog from "../ErrorDialog.vue";
import type { LauncherErrorPayload } from "@/lib/tauri";

describe("ErrorDialog", () => {
  it("does not render content when error is null", () => {
    const wrapper = mount(ErrorDialog, {
      props: { error: null, lastFailedAction: null },
    });
    expect(wrapper.text()).not.toContain("操作失败");
  });

  it("renders error message when error is provided", () => {
    const error: LauncherErrorPayload = {
      kind: "host",
      message: "spawn failed",
    };
    const wrapper = mount(ErrorDialog, {
      props: { error, lastFailedAction: "startHost" },
      global: {
        stubs: {
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
        },
      },
    });
    expect(wrapper.text()).toContain("操作失败");
    expect(wrapper.text()).toContain("spawn failed");
    // 重试按钮文案根据 lastFailedAction=startHost 显示"重试启动"
    expect(wrapper.text()).toContain("重试启动");
  });

  it("hides retry button when lastFailedAction is null", () => {
    const error: LauncherErrorPayload = { kind: "io", message: "boom" };
    const wrapper = mount(ErrorDialog, {
      props: { error, lastFailedAction: null },
      global: {
        stubs: {
          Dialog: {
            template: '<div v-if="open"><slot /></div>',
            props: ["open"],
          },
          DialogContent: { template: "<div><slot /></div>" },
          DialogHeader: { template: "<div><slot /></div>" },
          DialogTitle: { template: "<h2><slot /></h2>" },
          DialogDescription: { template: "<p><slot /></p>" },
          DialogFooter: { template: "<div><slot /></div>" },
        },
      },
    });
    const buttons = wrapper.findAll("button");
    // 只有"关闭"按钮，没有"重试"
    expect(buttons.some((b) => b.text().includes("重试"))).toBe(false);
  });

  it("emits dismiss when close button clicked", async () => {
    const error: LauncherErrorPayload = { kind: "io", message: "boom" };
    const wrapper = mount(ErrorDialog, {
      props: { error, lastFailedAction: "startHost" },
      global: {
        stubs: {
          Dialog: {
            template: '<div v-if="open"><slot /></div>',
            props: ["open"],
          },
          DialogContent: { template: "<div><slot /></div>" },
          DialogHeader: { template: "<div><slot /></div>" },
          DialogTitle: { template: "<h2><slot /></h2>" },
          DialogDescription: { template: "<p><slot /></p>" },
          DialogFooter: { template: "<div><slot /></div>" },
        },
      },
    });

    // 找到"关闭"按钮并点击
    const buttons = wrapper.findAll("button");
    const closeBtn = buttons.find((b) => b.text() === "关闭");
    expect(closeBtn).toBeDefined();
    await closeBtn!.trigger("click");

    expect(wrapper.emitted("dismiss")).toBeTruthy();
  });

  it("emits retry when retry button clicked", async () => {
    const error: LauncherErrorPayload = { kind: "io", message: "boom" };
    const wrapper = mount(ErrorDialog, {
      props: { error, lastFailedAction: "startHost" },
      global: {
        stubs: {
          Dialog: {
            template: '<div v-if="open"><slot /></div>',
            props: ["open"],
          },
          DialogContent: { template: "<div><slot /></div>" },
          DialogHeader: { template: "<div><slot /></div>" },
          DialogTitle: { template: "<h2><slot /></h2>" },
          DialogDescription: { template: "<p><slot /></p>" },
          DialogFooter: { template: "<div><slot /></div>" },
        },
      },
    });

    const buttons = wrapper.findAll("button");
    const retryBtn = buttons.find((b) => b.text().includes("重试"));
    expect(retryBtn).toBeDefined();
    await retryBtn!.trigger("click");

    expect(wrapper.emitted("retry")).toBeTruthy();
  });

  it("renders error data block when data is present", () => {
    const error: LauncherErrorPayload = {
      kind: "state_corrupt",
      message: "state file is corrupt",
      data: { path: "/tmp/state.json" },
    };
    const wrapper = mount(ErrorDialog, {
      props: { error, lastFailedAction: "startHost" },
      global: {
        stubs: {
          Dialog: {
            template: '<div v-if="open"><slot /></div>',
            props: ["open"],
          },
          DialogContent: { template: "<div><slot /></div>" },
          DialogHeader: { template: "<div><slot /></div>" },
          DialogTitle: { template: "<h2><slot /></h2>" },
          DialogDescription: { template: "<p><slot /></p>" },
          DialogFooter: { template: "<div><slot /></div>" },
        },
      },
    });

    expect(wrapper.text()).toContain("/tmp/state.json");
  });
});
