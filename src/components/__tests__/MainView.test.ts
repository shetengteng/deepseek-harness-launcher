import { flushPromises, mount, shallowMount } from "@vue/test-utils";
import type { Component } from "vue";
import { beforeEach, expect, test, vi } from "vitest";

type ToastOptions = {
  title?: string;
  action?: Component;
};

const api = vi.hoisted(() => ({
  checkDshUpdate: vi.fn(),
  installDsh: vi.fn(),
  restartHostAfterDshUpdate: vi.fn(),
}));

const notice = vi.hoisted(() => ({
  update: vi.fn(),
  dismiss: vi.fn(),
}));

const toastApi = vi.hoisted(() => ({
  toast: vi.fn(() => notice),
  ToastAction: {
    template: '<button v-bind="$attrs"><slot /></button>',
  },
}));

const store = vi.hoisted(() => ({
  phase: "ready",
  displayPhase: "ready",
  starting: false,
  crashRecovering: false,
  dshVersion: "0.1.0-rc.6",
  nodeVersion: "22.19.0",
  origin: "http://127.0.0.1:1337/",
  autoRestartedAttempt: null,
  error: null,
  lastFailedAction: null,
  crashLimit: null,
  refreshStatus: vi.fn(),
  initCrashEvents: vi.fn(),
  setHostReady: vi.fn(),
  startHost: vi.fn(),
  installDsh: vi.fn(),
  resetError: vi.fn(),
  retryLastAction: vi.fn(),
  retryAfterCrash: vi.fn(),
  rollbackAfterCrash: vi.fn(),
  dismissCrash: vi.fn(),
}));

vi.mock("@/lib/tauri", () => api);
vi.mock("@/components/ui/toast", () => toastApi);
vi.mock("@/stores/launcher", () => ({ useLauncherStore: () => store }));
vi.mock("@/composables/useTrayEvents", () => ({ useTrayEvents: vi.fn() }));

import MainView from "@/components/MainView.vue";

beforeEach(() => {
  vi.clearAllMocks();
  store.phase = "ready";
  store.displayPhase = "ready";
  store.starting = false;
  store.crashRecovering = false;
  store.origin = "http://127.0.0.1:1337/";
  api.checkDshUpdate.mockResolvedValue({
    current_version: "0.1.0-rc.6",
    latest_version: "0.1.0-rc.7",
  });
  api.installDsh.mockResolvedValue("0.1.0-rc.7");
  api.restartHostAfterDshUpdate.mockResolvedValue({
    origin: "http://127.0.0.1:1338/",
    active_version: "0.1.0-rc.7",
    rolled_back: false,
  });
});

test("installs and restarts immediately when the update toast action is clicked", async () => {
  shallowMount(MainView);
  await flushPromises();

  const [options] = toastApi.toast.mock.calls[0]! as unknown as [ToastOptions];
  expect(options.title).toBe("发现新版本");
  expect(options.action).toBeDefined();

  const action = mount(options.action!, {
    global: { stubs: { ToastAction: toastApi.ToastAction } },
  });
  await action.get("button").trigger("click");
  await flushPromises();

  expect(api.installDsh).toHaveBeenCalledWith({
    expectedVersion: "0.1.0-rc.7",
  });
  expect(api.restartHostAfterDshUpdate).toHaveBeenCalledOnce();
  expect(store.setHostReady).toHaveBeenCalledWith("http://127.0.0.1:1338/");
  expect(notice.dismiss).toHaveBeenCalledOnce();
});
