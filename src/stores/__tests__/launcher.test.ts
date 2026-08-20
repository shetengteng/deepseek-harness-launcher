import { beforeEach, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

vi.mock("@/lib/tauri", () => ({
  cancelDshInstall: vi.fn(),
  cancelNodeInstall: vi.fn(),
  fetchStatus: vi.fn(),
  getLatestDshVersion: vi.fn(),
  installDsh: vi.fn(),
  installNode: vi.fn(),
  listMirrors: vi.fn(),
  probeMirrors: vi.fn(),
  resolveBootstrapPlan: vi.fn(),
  restartHost: vi.fn(),
  rollbackDsh: vi.fn(),
  setRegistry: vi.fn(),
  shutdownHost: vi.fn(),
  startHost: vi.fn(),
  validateCustomMirror: vi.fn(),
}));

import { useLauncherStore } from "@/stores/launcher";

beforeEach(() => {
  setActivePinia(createPinia());
});

test("dsh runtime repair clears only the unavailable dsh version", () => {
  const store = useLauncherStore();
  store.phase = "error";
  store.nodeVersion = "22.19.0";
  store.dshVersion = "0.2.0";
  store.bootstrapPlan = {
    dsh_version: "0.2.0",
    registry: "https://registry.npmjs.org",
    engines_node: null,
    node_version: "22.19.0",
    requirement_source: "launcher-verified-fallback",
    resolved_at: "2026-08-17T00:00:00Z",
    phase: "resolved",
  };
  store.error = { kind: "dsh_not_installed", message: "missing entry" };

  store.resetError();

  expect(store.phase).toBe("first_run");
  expect(store.nodeVersion).toBe("22.19.0");
  expect(store.dshVersion).toBeNull();
  expect(store.bootstrapPlan).toBeNull();
});

test("node runtime repair clears Node and dsh state for a full reinstall", () => {
  const store = useLauncherStore();
  store.phase = "error";
  store.nodeVersion = "22.19.0";
  store.dshVersion = "0.2.0";
  store.error = { kind: "node_not_installed", message: "missing executable" };

  store.resetError();

  expect(store.phase).toBe("first_run");
  expect(store.nodeVersion).toBeNull();
  expect(store.dshVersion).toBeNull();
});

test("setHostReady remounts the host session even when origin is unchanged", () => {
  const store = useLauncherStore();
  store.setHostReady("http://127.0.0.1:1337/");
  const firstSession = store.hostSession;

  store.starting = true;
  store.setHostReady("http://127.0.0.1:1337/");

  expect(store.hostSession).toBe(firstSession + 1);
  expect(store.origin).toBe("http://127.0.0.1:1337/");
  expect(store.phase).toBe("ready");
  expect(store.starting).toBe(false);
});

test("tray restart failure clears the starting overlay and records a host error", () => {
  const store = useLauncherStore();
  store.phase = "ready";
  store.starting = true;

  store.failHostRestart("failed to spawn host");

  expect(store.starting).toBe(false);
  expect(store.phase).toBe("error");
  expect(store.error).toMatchObject({
    kind: "host",
    message: "failed to spawn host",
  });
  expect(store.lastFailedAction).toBe("startHost");
});
