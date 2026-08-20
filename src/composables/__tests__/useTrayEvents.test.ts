import { defineComponent } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const eventListeners = vi.hoisted(
  () => new Map<string, (event: { payload: unknown }) => void>(),
);

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, listener) => {
    eventListeners.set(eventName, listener);
    return Promise.resolve(vi.fn());
  }),
}));

import { useTrayEvents } from "@/composables/useTrayEvents";

const handlers = vi.hoisted(() => ({
  openSettings: vi.fn(),
  checkDshUpdate: vi.fn(),
  exportDiagnostics: vi.fn(),
  openAbout: vi.fn(),
  hostRestarting: vi.fn(),
  hostRestarted: vi.fn(),
  hostRestartFailed: vi.fn(),
}));

const Harness = defineComponent({
  setup() {
    useTrayEvents(handlers);
    return () => null;
  },
});

beforeEach(() => {
  vi.clearAllMocks();
  eventListeners.clear();
});

test("forwards tray host restart events with their payloads", async () => {
  mount(Harness);
  await flushPromises();

  eventListeners.get("tray-host-restarting")?.({ payload: null });
  eventListeners.get("tray-host-restarted")?.({
    payload: "http://127.0.0.1:1339/",
  });
  eventListeners.get("tray-host-restart-failed")?.({
    payload: "failed to spawn host",
  });

  expect(handlers.hostRestarting).toHaveBeenCalledOnce();
  expect(handlers.hostRestarted).toHaveBeenCalledWith(
    "http://127.0.0.1:1339/",
  );
  expect(handlers.hostRestartFailed).toHaveBeenCalledWith(
    "failed to spawn host",
  );
});
