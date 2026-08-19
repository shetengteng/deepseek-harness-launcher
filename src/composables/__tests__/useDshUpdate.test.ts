import { defineComponent } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() => ({
  cancelDshInstall: vi.fn(),
  cancelNodeInstall: vi.fn(),
  checkDshUpdate: vi.fn(),
  installDsh: vi.fn(),
  rollbackNodeUpgrade: vi.fn(),
  restartHostAfterDshUpdate: vi.fn(),
  upgradeNodeForDshUpdate: vi.fn(),
}));

const eventListeners = vi.hoisted(
  () => new Map<string, (event: { payload: { stage: string } }) => void>(),
);

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>(
    "@/lib/tauri",
  );
  return { ...actual, ...api };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, listener) => {
    eventListeners.set(eventName, listener);
    return Promise.resolve(vi.fn());
  }),
}));

vi.mock("@/components/ui/toast", () => ({
  toast: vi.fn(() => ({ dismiss: vi.fn() })),
}));

const store = vi.hoisted(() => ({
  displayPhase: "ready",
  dshVersion: "0.1.0",
  nodeVersion: "22.19.0",
  setHostReady: vi.fn(),
}));

vi.mock("@/stores/launcher", () => ({
  useLauncherStore: () => store,
}));

import { useDshUpdate } from "@/composables/useDshUpdate";

const Harness = defineComponent({
  setup() {
    return useDshUpdate();
  },
  template: "<div />",
});

function nodeUpgradeError() {
  return {
    kind: "node_upgrade_required",
    message: "dsh 0.2.0 requires Node >=24.0.0, current version is 22.19.0",
    data: {
      dsh_version: "0.2.0",
      current_node: "22.19.0",
      engines_node: ">=24.0.0",
      suggested_node: "24.4.0",
    },
  };
}

function nodeUpgradeTransaction() {
  return {
    upgraded_node: "24.4.0",
    previous_node: {
      version: "22.19.0",
      installed_at: "2026-08-19T00:00:00Z",
      mirror: "https://nodejs.org/dist",
    },
    previous_node_mirror: "https://nodejs.org/dist",
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  eventListeners.clear();
  store.displayPhase = "ready";
  store.dshVersion = "0.1.0";
  store.nodeVersion = "22.19.0";
  api.checkDshUpdate.mockResolvedValue({
    current_version: "0.1.0",
    latest_version: "0.2.0",
  });
  api.restartHostAfterDshUpdate.mockResolvedValue({
    origin: "http://127.0.0.1:1337/",
    active_version: "0.2.0",
    rolled_back: false,
  });
  api.rollbackNodeUpgrade.mockResolvedValue("22.19.0");
});

test("prompts for a Node upgrade and leaves the current runtime when cancelled", async () => {
  api.installDsh.mockRejectedValueOnce(nodeUpgradeError());
  const wrapper = mount(Harness);
  await flushPromises();

  wrapper.vm.startDshUpdate();
  await flushPromises();

  expect(wrapper.vm.updateDialogState).toBe("confirming_node");
  expect(wrapper.vm.nodeUpgrade?.suggested_node).toBe("24.4.0");
  expect(api.upgradeNodeForDshUpdate).not.toHaveBeenCalled();

  await wrapper.vm.cancelDshUpdate();
  await flushPromises();

  expect(wrapper.vm.updateDialogState).toBe("idle");
  expect(api.upgradeNodeForDshUpdate).not.toHaveBeenCalled();
  expect(store.nodeVersion).toBe("22.19.0");
});

test("upgrades Node after confirmation then installs and restarts dsh", async () => {
  api.installDsh
    .mockRejectedValueOnce(nodeUpgradeError())
    .mockResolvedValueOnce("0.2.0");
  api.upgradeNodeForDshUpdate.mockResolvedValue(nodeUpgradeTransaction());
  const wrapper = mount(Harness);
  await flushPromises();

  wrapper.vm.startDshUpdate();
  await flushPromises();
  await wrapper.vm.confirmNodeUpgrade();
  await flushPromises();

  expect(api.upgradeNodeForDshUpdate).toHaveBeenCalledWith({
    version: "24.4.0",
    operationId: expect.any(String),
  });
  expect(api.installDsh).toHaveBeenNthCalledWith(2, {
    operationId: expect.any(String),
    expectedVersion: "0.2.0",
  });
  expect(api.restartHostAfterDshUpdate).toHaveBeenCalledOnce();
  expect(store.nodeVersion).toBe("24.4.0");
  expect(store.setHostReady).toHaveBeenCalledWith("http://127.0.0.1:1337/");
  expect(wrapper.vm.updateDialogState).toBe("idle");
});

test("rolls Node back when dsh installation fails after the upgrade", async () => {
  api.installDsh
    .mockRejectedValueOnce(nodeUpgradeError())
    .mockRejectedValueOnce({ message: "npm install failed" });
  api.upgradeNodeForDshUpdate.mockResolvedValue(nodeUpgradeTransaction());
  const wrapper = mount(Harness);
  await flushPromises();

  wrapper.vm.startDshUpdate();
  await flushPromises();
  await wrapper.vm.confirmNodeUpgrade();
  await flushPromises();

  expect(api.rollbackNodeUpgrade).toHaveBeenCalledWith(nodeUpgradeTransaction());
  expect(store.nodeVersion).toBe("22.19.0");
  expect(wrapper.vm.updateDialogState).toBe("failed");
});

test("rolls Node back when dsh installation is cancelled", async () => {
  api.installDsh
    .mockRejectedValueOnce(nodeUpgradeError())
    .mockRejectedValueOnce({
      kind: "dsh_install_cancelled",
      message: "dsh installation was cancelled",
    });
  api.upgradeNodeForDshUpdate.mockResolvedValue(nodeUpgradeTransaction());
  const wrapper = mount(Harness);
  await flushPromises();

  wrapper.vm.startDshUpdate();
  await flushPromises();
  await wrapper.vm.confirmNodeUpgrade();
  await flushPromises();

  expect(api.rollbackNodeUpgrade).toHaveBeenCalledWith(nodeUpgradeTransaction());
  expect(store.nodeVersion).toBe("22.19.0");
  expect(wrapper.vm.updateDialogState).toBe("idle");
});

test("reports npm package activity while downloading an update", async () => {
  let finishInstall: (version: string) => void = () => undefined;
  api.installDsh.mockImplementationOnce(
    () =>
      new Promise<string>((resolve) => {
        finishInstall = resolve;
      }),
  );
  const wrapper = mount(Harness);
  await flushPromises();

  wrapper.vm.startDshUpdate();
  await flushPromises();

  eventListeners.get("dsh-install-progress")?.({
    payload: { stage: "downloading" },
  });
  eventListeners.get("dsh-install-progress")?.({
    payload: { stage: "downloading" },
  });

  expect(wrapper.vm.updateInstallActivity).toBe(2);
  expect(wrapper.vm.updateStageMessage).toBe(
    "npm install 进行中，已处理 2 个包…",
  );

  finishInstall("0.2.0");
  await flushPromises();
  wrapper.unmount();
});
