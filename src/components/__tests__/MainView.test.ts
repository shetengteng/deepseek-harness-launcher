import { flushPromises, mount, shallowMount } from "@vue/test-utils";
import type { Component } from "vue";
import { beforeEach, expect, test, vi } from "vitest";

type ToastOptions = {
  title?: string;
  action?: Component;
};

const api = vi.hoisted(() => ({
  cancelDshInstall: vi.fn(),
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
}));

const opener = vi.hoisted(() => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
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
vi.mock("@tauri-apps/plugin-opener", () => opener);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));
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
  api.cancelDshInstall.mockResolvedValue(true);
  api.installDsh.mockResolvedValue("0.1.0-rc.7");
  api.restartHostAfterDshUpdate.mockResolvedValue({
    origin: "http://127.0.0.1:1338/",
    active_version: "0.1.0-rc.7",
    rolled_back: false,
  });
});

test("opens the update dialog, then installs and restarts when the toast action is clicked", async () => {
  let finishInstall: (version: string) => void = () => undefined;
  api.installDsh.mockImplementationOnce(
    () =>
      new Promise<string>((resolve) => {
        finishInstall = resolve;
      }),
  );
  const wrapper = mount(MainView, { attachTo: document.body });
  await flushPromises();

  const [options] = toastApi.toast.mock.calls[0]! as unknown as [ToastOptions];
  expect(options.title).toBe("发现新版本");
  expect(options.action).toBeDefined();

  const action = mount(options.action!);
  await action.get("button").trigger("click");
  await flushPromises();

  expect(document.body.textContent).toContain("正在更新 dsh");
  expect(api.installDsh).toHaveBeenCalledWith({
    operationId: expect.any(String),
    expectedVersion: "0.1.0-rc.7",
  });
  finishInstall("0.1.0-rc.7");
  await flushPromises();
  expect(api.restartHostAfterDshUpdate).toHaveBeenCalledOnce();
  expect(store.setHostReady).toHaveBeenCalledWith("http://127.0.0.1:1338/");
  expect(notice.dismiss).toHaveBeenCalledOnce();
  wrapper.unmount();
});

test("cancels the active update from the dialog", async () => {
  let rejectInstall: (error: unknown) => void = () => undefined;
  api.installDsh.mockImplementationOnce(
    () =>
      new Promise<string>((_resolve, reject) => {
        rejectInstall = reject;
      }),
  );
  const wrapper = mount(MainView, { attachTo: document.body });
  await flushPromises();

  const [options] = toastApi.toast.mock.calls[0]! as unknown as [ToastOptions];
  const action = mount(options.action!);
  await action.get("button").trigger("click");
  await flushPromises();

  const cancelButton = [...document.querySelectorAll("button")].find(
    (button) => button.textContent?.trim() === "取消",
  );
  expect(cancelButton).toBeDefined();
  cancelButton?.click();
  await flushPromises();

  expect(api.cancelDshInstall).toHaveBeenCalledWith(expect.any(String));
  rejectInstall({ message: "dsh installation was cancelled" });
  await flushPromises();
  expect(document.body.textContent).not.toContain("正在更新 dsh");
  wrapper.unmount();
});

test("retries the same displayed version after an update failure", async () => {
  api.installDsh
    .mockRejectedValueOnce({ message: "registry temporarily unavailable" })
    .mockResolvedValueOnce("0.1.0-rc.7");
  const wrapper = mount(MainView, { attachTo: document.body });
  await flushPromises();

  const [options] = toastApi.toast.mock.calls[0]! as unknown as [ToastOptions];
  const action = mount(options.action!);
  await action.get("button").trigger("click");
  await flushPromises();

  const retryButton = [...document.querySelectorAll("button")].find(
    (button) => button.textContent?.trim() === "重试",
  );
  expect(retryButton).toBeDefined();
  retryButton?.click();
  await flushPromises();

  expect(api.installDsh).toHaveBeenNthCalledWith(1, {
    operationId: expect.any(String),
    expectedVersion: "0.1.0-rc.7",
  });
  expect(api.installDsh).toHaveBeenNthCalledWith(2, {
    operationId: expect.any(String),
    expectedVersion: "0.1.0-rc.7",
  });
  wrapper.unmount();
});

test("opens a verified dsh external link in the system browser", async () => {
  const wrapper = shallowMount(MainView);
  await flushPromises();
  const frame = wrapper.get("iframe").element as HTMLIFrameElement;

  window.dispatchEvent(
    new MessageEvent("message", {
      origin: "http://127.0.0.1:1337",
      source: frame.contentWindow,
      data: {
        type: "dsh:open-external",
        href: "https://docs.deepseek.com/guide",
      },
    }),
  );
  await flushPromises();

  expect(opener.openUrl).toHaveBeenCalledWith("https://docs.deepseek.com/guide");
});

test("rejects external-link messages not sent by the active dsh iframe", async () => {
  const wrapper = shallowMount(MainView);
  await flushPromises();
  const frame = wrapper.get("iframe").element as HTMLIFrameElement;

  window.dispatchEvent(
    new MessageEvent("message", {
      origin: "https://example.com",
      source: frame.contentWindow,
      data: {
        type: "dsh:open-external",
        href: "https://docs.deepseek.com/guide",
      },
    }),
  );
  window.dispatchEvent(
    new MessageEvent("message", {
      origin: "http://127.0.0.1:1337",
      source: frame.contentWindow,
      data: {
        type: "dsh:open-external",
        href: "file:///Users/example/.ssh/id_rsa",
      },
    }),
  );
  await flushPromises();

  expect(opener.openUrl).not.toHaveBeenCalled();
});

test("keeps dsh browser permissions and popup capability available", async () => {
  const wrapper = shallowMount(MainView);
  await flushPromises();
  const frame = wrapper.get("iframe");

  expect(frame.attributes("allow")).toContain("camera");
  expect(frame.attributes("allow")).toContain("microphone");
  expect(frame.attributes("allow")).toContain("geolocation");
  expect(frame.attributes("sandbox")).toContain("allow-popups");
  expect(frame.attributes("sandbox")).toContain("allow-modals");
});
