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
