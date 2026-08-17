import { flushPromises, shallowMount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const store = vi.hoisted(() => ({
  nodeVersion: null as string | null,
  dshVersion: null as string | null,
  wizardStep: "downloading",
  installing: true,
  installingDsh: false,
  nodeInstallOperationId: "download-1" as string | null,
  dshInstallOperationId: null as string | null,
  bootstrapPlan: {
    node_version: "22.19.0",
    dsh_version: "0.1.0",
    registry: "https://registry.npmjs.org",
  },
  latestDshVersion: null,
  downloadPercent: 42,
  dshInstallProgress: 0,
  dshInstallActivity: 0,
  dshInstallStage: "resolving",
  startBootstrap: vi.fn(),
  restartNodeDownload: vi.fn(),
  restartDshInstall: vi.fn(),
  applyProgressEvent: vi.fn(),
  applyDshInstallProgress: vi.fn(),
}));

vi.mock("@/stores/launcher", () => ({ useLauncherStore: () => store }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

import FirstRun from "@/components/FirstRun.vue";

const stubs = {
  Button: {
    emits: ["click"],
    template: "<button v-bind=\"$attrs\" @click=\"$emit('click')\"><slot /></button>",
  },
  Progress: { template: "<div />" },
  MirrorSelector: { template: '<div data-testid="mirror-selector" />' },
};

beforeEach(() => {
  vi.clearAllMocks();
  store.nodeVersion = null;
  store.dshVersion = null;
  store.wizardStep = "downloading";
  store.installing = true;
  store.installingDsh = false;
  store.nodeInstallOperationId = "download-1";
  store.dshInstallOperationId = null;
  store.bootstrapPlan.registry = "https://registry.npmjs.org";
});

test("starts bootstrap installation automatically on first run", async () => {
  shallowMount(FirstRun, { global: { stubs } });
  await flushPromises();

  expect(store.startBootstrap).toHaveBeenCalledOnce();
});

test("shows advanced mirror controls without opening them by default", async () => {
  const wrapper = shallowMount(FirstRun, { global: { stubs } });
  await flushPromises();

  const advanced = wrapper.find("details");
  expect(advanced.attributes("open")).toBeUndefined();
  expect(advanced.text()).toContain("切换下载来源");
  expect(advanced.find('[data-testid="mirror-selector"]').exists()).toBe(true);
});

test("restarts the active Node installation during extraction", async () => {
  store.wizardStep = "extracting";
  const wrapper = shallowMount(FirstRun, { global: { stubs } });
  await flushPromises();

  await wrapper
    .findAll("button")
    .find((button) => button.text().includes("重新使用此来源下载"))!
    .trigger("click");

  expect(store.restartNodeDownload).toHaveBeenCalledOnce();
});

test("allows switching the npm source while dsh is installing", async () => {
  store.nodeVersion = "22.19.0";
  store.installing = false;
  store.installingDsh = true;
  store.dshInstallOperationId = "dsh-install-1";
  const wrapper = shallowMount(FirstRun, { global: { stubs } });
  await flushPromises();

  const advanced = wrapper.find("details");
  expect(advanced.text()).toContain("切换 npm 下载源");
  expect(advanced.text()).toContain("npm 下载源");
  expect(wrapper.text()).toContain("DeepSeek Harness");
  expect(wrapper.text()).not.toContain("@deepseek-ai/dsh");
  expect(advanced.find('[data-testid="mirror-selector"]').exists()).toBe(false);

  await wrapper
    .findAll("button")
    .find((button) => button.text().includes("重新使用此 npm 来源下载"))!
    .trigger("click");

  expect(store.restartDshInstall).toHaveBeenCalledWith(
    "https://registry.npmjs.org",
  );
});

test("keeps npm restart available after the installation task stops", async () => {
  store.nodeVersion = "22.19.0";
  store.installing = false;
  store.installingDsh = false;
  store.dshInstallOperationId = null;
  const wrapper = shallowMount(FirstRun, { global: { stubs } });
  await flushPromises();

  const restart = wrapper
    .findAll("button")
    .find((button) => button.text().includes("重新使用此 npm 来源下载"))!;
  expect(restart.attributes("disabled")).toBeUndefined();

  await restart.trigger("click");
  expect(store.restartDshInstall).toHaveBeenCalledWith(
    "https://registry.npmjs.org",
  );
});
