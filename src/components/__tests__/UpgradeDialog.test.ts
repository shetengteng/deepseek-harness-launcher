// UpgradeDialog.vue 组件测试。对应测试设计 §PR-016。
// 覆盖：打开显示版本信息、稍后按钮、重启按钮。

import { beforeEach, describe, expect, it, vi, afterEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import UpgradeDialog from "../UpgradeDialog.vue";

// reka-ui Dialog 在 jsdom 下通过 teleport 渲染，stub 掉
const dialogStubs = {
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

beforeEach(() => {
  setActivePinia(createPinia());
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("UpgradeDialog", () => {
  it("renders when open with version", () => {
    const wrapper = mount(UpgradeDialog, {
      props: { open: true, version: "0.2.0", upgrading: false },
      global: { stubs: dialogStubs },
    });

    expect(wrapper.text()).toContain("升级就绪");
    expect(wrapper.text()).toContain("0.2.0");
    expect(wrapper.text()).toContain("已安装，重启后生效");
  });

  it("renders without version", () => {
    const wrapper = mount(UpgradeDialog, {
      props: { open: true, version: null, upgrading: false },
      global: { stubs: dialogStubs },
    });

    expect(wrapper.text()).toContain("新版本已安装，重启后生效");
  });

  it("has restart and later buttons", () => {
    const wrapper = mount(UpgradeDialog, {
      props: { open: true, version: "0.2.0", upgrading: false },
      global: { stubs: dialogStubs },
    });

    const buttons = wrapper.findAll("button");
    const laterBtn = buttons.find((b) => b.text().includes("稍后"));
    const restartBtn = buttons.find((b) => b.text().includes("重启生效"));

    expect(laterBtn).toBeDefined();
    expect(restartBtn).toBeDefined();
  });

  it("disables restart button when upgrading", () => {
    const wrapper = mount(UpgradeDialog, {
      props: { open: true, version: "0.2.0", upgrading: true },
      global: { stubs: dialogStubs },
    });

    const restartBtn = wrapper.findAll("button").find((b) =>
      b.text().includes("重启中"),
    );
    expect(restartBtn?.attributes("disabled")).toBeDefined();
  });

  it("emits later when later button clicked", async () => {
    const wrapper = mount(UpgradeDialog, {
      props: { open: true, version: "0.2.0", upgrading: false },
      global: { stubs: dialogStubs },
    });

    const laterBtn = wrapper.findAll("button").find((b) =>
      b.text().includes("稍后"),
    );
    await laterBtn?.trigger("click");

    expect(wrapper.emitted("later")).toBeTruthy();
  });

  it("emits restart when restart button clicked", async () => {
    const wrapper = mount(UpgradeDialog, {
      props: { open: true, version: "0.2.0", upgrading: false },
      global: { stubs: dialogStubs },
    });

    const restartBtn = wrapper.findAll("button").find((b) =>
      b.text().includes("重启生效"),
    );
    await restartBtn?.trigger("click");

    expect(wrapper.emitted("restart")).toBeTruthy();
  });

  it("hides content when open is false", () => {
    const wrapper = mount(UpgradeDialog, {
      props: { open: false, version: "0.2.0", upgrading: false },
      global: { stubs: dialogStubs },
    });

    expect(wrapper.text()).not.toContain("升级就绪");
  });
});