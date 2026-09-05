import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() =>
  Object.fromEntries(
    [
      "getDshState",
      "getLatestDshVersion",
      "getNodeUpdateTarget",
      "installDshCli",
      "installDsh",
      "cancelDshInstall",
      "cancelNodeInstall",
      "upgradeNode",
      "upgradeNodeForDshUpdate",
      "rollbackNodeUpgrade",
      "restartHostAfterDshUpdate",
      "setNodeMirror",
      "setRegistry",
      "listMirrors",
      "exportDiagnostics",
      "getDshCliStatus",
      "uninstallManagedRuntime",
      "uninstallDshCli",
    ].map((name) => [name, vi.fn()]),
  ),
);
const eventListeners = vi.hoisted(
  () =>
    new Map<
      string,
      (event: {
        payload: { stage: string; bytes: number; total: number | null };
      }) => void
    >(),
);
const theme = vi.hoisted(() => ({
  mode: "dark" as "light" | "dark",
  initializing: false,
  saving: false,
  error: null as string | null,
  updateTheme: vi.fn(),
}));

vi.mock("@/lib/tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/tauri")>();
  return { ...actual, ...api };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (
      event: string,
      handler: (event: {
        payload: { stage: string; bytes: number; total: number | null };
      }) => void,
    ) => {
      eventListeners.set(event, handler);
      return Promise.resolve(vi.fn());
    },
  ),
}));

vi.mock("@/stores/theme", () => ({ useThemeStore: () => theme }));

import Settings from "@/components/Settings.vue";

function button(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper
    .findAll("button")
    .find((item) => item.text().trim() === label)!;
}

function state(current: string | null) {
  return {
    current,
    known_good: null,
    installed: [],
    node_mirror: "https://nodejs.org/dist",
    registry: "https://registry.npmjs.org",
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

function dshCliStatus(
  state: "installed" | "not_installed" | "conflict" = "not_installed",
) {
  return {
    state,
    command_path: "/Users/test/.local/bin/dsh",
    path_instruction: "关闭并重新打开 Terminal。",
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  eventListeners.clear();
  api.getDshState.mockResolvedValue(state("0.0.9"));
  api.listMirrors.mockResolvedValue([]);
  api.getLatestDshVersion.mockResolvedValue({ latest_version: "0.1.0" });
  api.restartHostAfterDshUpdate.mockResolvedValue({
    origin: "http://127.0.0.1:1337/",
    active_version: "0.1.0",
    rolled_back: false,
  });
  api.getDshCliStatus.mockResolvedValue(dshCliStatus());
  theme.mode = "dark";
  theme.initializing = false;
  theme.saving = false;
  theme.error = null;
});

test("uses the same horizontal gutters as plugin commands", async () => {
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.get(".settings-panel").classes()).toEqual(
    expect.arrayContaining([
      "px-8",
      "max-sm:px-[18px]",
      "pt-[clamp(32px,8vh,88px)]",
      "max-sm:pt-7",
    ]),
  );
  expect(wrapper.find(".settings-panel-content").exists()).toBe(true);
});

test("introduces the settings workspace before its controls", async () => {
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.get(".settings-panel-heading").text()).toContain("设置");
  expect(wrapper.get(".settings-panel-heading").text()).toContain(
    "管理 DeepSeek Harness 的运行时",
  );
});

test("changes the launcher theme from settings", async () => {
  const wrapper = mount(Settings);
  await flushPromises();

  await wrapper
    .get('[data-testid="black-white-theme-switch"]')
    .trigger("click");

  expect(theme.updateTheme).toHaveBeenCalledWith("light");
});

test("requires explicit confirmation before reinstalling managed runtime", async () => {
  api.uninstallManagedRuntime.mockResolvedValue(undefined);
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "重新安装").trigger("click");
  expect(api.uninstallManagedRuntime).not.toHaveBeenCalled();

  await button(wrapper, "清除并退出").trigger("click");
  expect(api.uninstallManagedRuntime).toHaveBeenCalledTimes(1);
});

test("updates only to the registry latest version after explicit confirmation", async () => {
  api.installDsh.mockResolvedValue("0.1.0");
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).toContain("0.1.0");
  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();

  expect(api.installDsh).toHaveBeenCalledWith({
    expectedVersion: "0.1.0",
    operationId: expect.any(String),
  });
  expect(api.restartHostAfterDshUpdate).toHaveBeenCalledOnce();
  expect(wrapper.emitted("upgradeReady")?.[0]).toEqual([
    "http://127.0.0.1:1337/",
  ]);
});

test("shows install progress details while updating to the latest dsh", async () => {
  let finishInstall: ((version: string) => void) | undefined;
  api.installDsh.mockImplementation(
    () => new Promise<string>((resolve) => (finishInstall = resolve)),
  );
  const wrapper = mount(Settings, { attachTo: document.body });
  await flushPromises();

  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();

  expect(document.body.textContent).toContain("正在更新 dsh");
  expect(document.body.textContent).toContain("0.0.9");
  expect(document.body.textContent).toContain("0.1.0");
  expect(
    document.body.querySelector('[data-testid="dsh-update-progress-status"]')
      ?.textContent,
  ).toContain("正在从当前下载源获取最新版本");

  eventListeners.get("dsh-install-progress")?.({
    payload: { stage: "downloading", bytes: 0, total: null },
  });
  await flushPromises();

  expect(
    document.body.querySelector('[data-testid="dsh-update-progress-status"]')
      ?.textContent,
  ).toContain("npm install 进行中，已处理 1 个包");

  finishInstall?.("0.1.0");
  await flushPromises();
  wrapper.unmount();
});

test("shows the start error after a new version fails and rolls back", async () => {
  api.installDsh.mockResolvedValue("0.1.0");
  api.restartHostAfterDshUpdate.mockResolvedValue({
    origin: "http://127.0.0.1:1337/",
    active_version: "0.1.1-rc.2",
    rolled_back: true,
    start_error: {
      kind: "host",
      message:
        "host supervisor error: desktop Host readiness timed out after 90s",
      user_message:
        "dsh 启动超时（90 秒内未就绪）。请重试；若持续失败请导出诊断信息。",
    },
  });
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();

  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "新版本无法启动，已恢复 0.1.1-rc.2。",
  );
  const startError = wrapper.get('[data-testid="dsh-start-error"]');
  expect(startError.text()).toContain("启动失败原因");
  expect(startError.text()).toContain("dsh 启动超时");
  expect(startError.text()).toContain("readiness timed out");
});

test("keeps the update status height fixed when the current version is latest", async () => {
  api.getDshState.mockResolvedValue(state("0.1.0"));
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).not.toContain("可更新版本");
  expect(wrapper.text()).not.toContain("安装新版本");
  expect(wrapper.text()).toContain("正在使用的 DeepSeek Harness 版本");
  expect(wrapper.text()).not.toContain("当前启动的 DeepSeek Harness");
  expect(wrapper.text()).toContain("已是最新版本");
  expect(wrapper.find('[data-testid="dsh-update-status"]').classes()).toContain(
    "text-xs",
  );
  expect(api.installDsh).not.toHaveBeenCalled();

  await button(wrapper, "刷新").trigger("click");
  await flushPromises();

  expect(api.getLatestDshVersion).toHaveBeenCalledTimes(2);
});

test("shows the current DeepSeek Harness IP address and port", async () => {
  const wrapper = mount(Settings, {
    props: { hostOrigin: "http://127.0.0.1:51842/" },
  });
  await flushPromises();

  expect(wrapper.text()).toContain("运行 IP 与端口");
  expect(wrapper.text()).toContain("127.0.0.1:51842");
});

test("updates only Node after the manual confirmation", async () => {
  api.getNodeUpdateTarget.mockResolvedValue({
    current_version: "22.19.0",
    target_version: "24.4.0",
    engines_node: ">=22.0.0",
    target_source: "dsh-engines",
    update_available: true,
  });
  api.upgradeNode.mockResolvedValue("24.4.0");
  const wrapper = mount(Settings, {
    attachTo: document.body,
    props: { nodeVersion: "22.19.0" },
  });
  await flushPromises();

  await button(wrapper, "更新 Node").trigger("click");
  await flushPromises();

  expect(api.getNodeUpdateTarget).toHaveBeenCalledOnce();
  expect(document.body.textContent).toContain("更新 Node.js");
  expect(document.body.textContent).toContain("24.4.0");
  expect(api.upgradeNode).not.toHaveBeenCalled();

  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "仅更新 Node")
    ?.click();
  await flushPromises();

  expect(api.upgradeNode).toHaveBeenCalledWith({
    version: "24.4.0",
    operationId: expect.any(String),
  });
  expect(api.installDsh).not.toHaveBeenCalled();
  expect(api.restartHostAfterDshUpdate).not.toHaveBeenCalled();
  expect(wrapper.emitted("nodeUpdated")?.[0]).toEqual(["24.4.0"]);
  wrapper.unmount();
});

test("shows download progress while manually updating Node", async () => {
  api.getNodeUpdateTarget.mockResolvedValue({
    current_version: "22.19.0",
    target_version: "24.4.0",
    engines_node: ">=22.0.0",
    target_source: "dsh-engines",
    update_available: true,
  });
  let finishUpgrade: ((version: string) => void) | undefined;
  api.upgradeNode.mockImplementation(
    () => new Promise<string>((resolve) => (finishUpgrade = resolve)),
  );
  const wrapper = mount(Settings, {
    attachTo: document.body,
    props: { nodeVersion: "22.19.0" },
  });
  await flushPromises();

  await button(wrapper, "更新 Node").trigger("click");
  await flushPromises();
  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "仅更新 Node")
    ?.click();
  await flushPromises();

  eventListeners.get("download-progress")?.({
    payload: { stage: "download", bytes: 50, total: 100 },
  });
  await flushPromises();

  expect(document.body.textContent).toContain("正在下载并校验 Node.js 运行时");
  expect(document.body.textContent).toContain("45%");

  finishUpgrade?.("24.4.0");
  await flushPromises();
  wrapper.unmount();
});

test("allows a verified fallback Node runtime to be reinstalled", async () => {
  api.getNodeUpdateTarget.mockResolvedValue({
    current_version: "22.19.0",
    target_version: "24.18.1",
    engines_node: null,
    target_source: "launcher-verified-fallback",
    update_available: true,
  });
  api.upgradeNode.mockResolvedValue("24.18.1");
  const wrapper = mount(Settings, {
    attachTo: document.body,
    props: { nodeVersion: "22.19.0" },
  });
  await flushPromises();

  await button(wrapper, "更新 Node").trigger("click");
  await flushPromises();

  expect(document.body.textContent).toContain("launcher 已验证的版本");
  expect(document.body.textContent).toContain("仅更新 Node");

  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "仅更新 Node")
    ?.click();
  await flushPromises();

  expect(api.upgradeNode).toHaveBeenCalledWith({
    version: "24.18.1",
    operationId: expect.any(String),
  });
  expect(api.installDsh).not.toHaveBeenCalled();
  wrapper.unmount();
});

test("installs the dsh command and shows the PATH instructions", async () => {
  api.installDshCli.mockResolvedValue({
    command_path: "/Users/test/.local/bin/dsh",
    path_instruction: "关闭并重新打开 Terminal。",
  });
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "安装命令").trigger("click");
  await flushPromises();

  expect(api.installDshCli).toHaveBeenCalledOnce();
  expect(wrapper.text()).toContain("/Users/test/.local/bin/dsh");
  expect(wrapper.text()).toContain("关闭并重新打开 Terminal。");
});

test("removes only the launcher-managed dsh command", async () => {
  api.getDshCliStatus
    .mockResolvedValueOnce(dshCliStatus("installed"))
    .mockResolvedValueOnce(dshCliStatus());
  api.uninstallDshCli.mockResolvedValue({
    command_path: "/Users/test/.local/bin/dsh",
    removed: true,
  });
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).toContain("dsh 命令已安装");
  await button(wrapper, "移除命令").trigger("click");
  await flushPromises();

  expect(api.uninstallDshCli).toHaveBeenCalledOnce();
  expect(wrapper.text()).toContain("安装 dsh 命令");
});

test("shows an actionable error when the dsh command cannot be installed", async () => {
  api.installDshCli.mockRejectedValue({ message: "目标已有其他 dsh 命令" });
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "安装命令").trigger("click");
  await flushPromises();

  expect(wrapper.text()).toContain("目标已有其他 dsh 命令");
});

test("does not expose the automatic recovery version in settings", async () => {
  api.getDshState.mockResolvedValue({
    ...state("0.0.9"),
    known_good: "0.0.8",
  });
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).not.toContain("可恢复的版本");
  expect(wrapper.text()).not.toContain("0.0.8");
});

test("retains the displayed current version when installing latest fails", async () => {
  api.installDsh.mockRejectedValue({
    message: "网络中断，请检查 npm 下载源后重试。",
  });
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();

  expect(wrapper.text()).toContain("0.0.9");
  expect(wrapper.text()).toContain("网络中断，请检查 npm 下载源后重试。");
  expect(wrapper.emitted("upgradeReady")).toBeUndefined();
});

test("asks to confirm a Node upgrade before changing the current runtime", async () => {
  api.installDsh.mockRejectedValueOnce({
    kind: "node_upgrade_required",
    message: "dsh 0.2.0 requires Node >=24.0.0",
    data: {
      dsh_version: "0.1.0",
      current_node: "22.19.0",
      engines_node: ">=24.0.0",
      suggested_node: "24.4.0",
    },
  });
  const wrapper = mount(Settings, { attachTo: document.body });
  await flushPromises();

  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();

  expect(api.upgradeNode).not.toHaveBeenCalled();
  expect(document.body.textContent).toContain("需要升级 Node");
  expect(document.body.textContent).toContain("24.4.0");
  expect(wrapper.text()).toContain("0.0.9");

  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "取消更新")
    ?.click();
  await flushPromises();

  expect(api.upgradeNode).not.toHaveBeenCalled();
  expect(api.installDsh).toHaveBeenCalledTimes(1);
  expect(document.body.textContent).not.toContain("需要升级 Node");
  expect(wrapper.text()).toContain("0.0.9");
  wrapper.unmount();
});

test("upgrades Node after confirmation and then installs the displayed dsh", async () => {
  api.installDsh
    .mockRejectedValueOnce({
      kind: "node_upgrade_required",
      message: "dsh 0.1.0 requires Node >=24.0.0",
      data: {
        dsh_version: "0.1.0",
        current_node: "22.19.0",
        engines_node: ">=24.0.0",
        suggested_node: "24.4.0",
      },
    })
    .mockResolvedValueOnce("0.1.0");
  api.upgradeNodeForDshUpdate.mockResolvedValue(nodeUpgradeTransaction());
  const wrapper = mount(Settings, { attachTo: document.body });
  await flushPromises();

  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();
  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "确认升级并继续")
    ?.click();
  await flushPromises();

  expect(api.upgradeNodeForDshUpdate).toHaveBeenCalledWith({
    version: "24.4.0",
    operationId: expect.any(String),
  });
  expect(api.installDsh).toHaveBeenNthCalledWith(2, {
    expectedVersion: "0.1.0",
    operationId: expect.any(String),
  });
  expect(api.restartHostAfterDshUpdate).toHaveBeenCalledOnce();
  expect(wrapper.emitted("upgradeReady")?.[0]).toEqual([
    "http://127.0.0.1:1337/",
  ]);
  wrapper.unmount();
});

test("shows local settings while the latest version check is still loading", async () => {
  api.getLatestDshVersion.mockReturnValue(new Promise(() => undefined));
  const wrapper = mount(Settings, {
    props: { nodeVersion: "22.19.0", hostOrigin: "http://127.0.0.1:51842/" },
  });
  await flushPromises();

  expect(wrapper.text()).toContain("外观");
  expect(wrapper.text()).toContain("问题排查");
  expect(wrapper.text()).toContain("0.0.9");
  expect(wrapper.text()).toContain("22.19.0");
  expect(wrapper.text()).toContain("127.0.0.1:51842");
  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "正在检查更新",
  );
  expect(wrapper.text()).not.toContain("无法加载设置");
});

test("keeps settings visible when the latest version check fails", async () => {
  api.getLatestDshVersion.mockRejectedValue({
    message: "npm registry unreachable",
  });
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).toContain("外观");
  expect(wrapper.text()).toContain("0.0.9");
  expect(wrapper.text()).toContain("问题排查");
  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "无法检查最新版本",
  );
  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "npm registry unreachable",
  );
  expect(wrapper.text()).not.toContain("无法加载设置");
});

test("shows appearance and support when runtime state cannot be loaded", async () => {
  api.getDshState.mockRejectedValue({ message: "state.json unreadable" });
  const wrapper = mount(Settings, {
    props: { nodeVersion: "22.19.0" },
  });
  await flushPromises();

  expect(wrapper.text()).toContain("外观");
  expect(wrapper.text()).toContain("问题排查");
  expect(wrapper.text()).toContain("22.19.0");
  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "无法读取运行环境",
  );
  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "state.json unreadable",
  );
  expect(wrapper.text()).toContain("无法加载下载来源");
  expect(wrapper.text()).not.toContain("无法加载设置");
});

test("loads the command card independently of the latest version check", async () => {
  api.getLatestDshVersion.mockReturnValue(new Promise(() => undefined));
  api.getDshCliStatus.mockReturnValue(new Promise(() => undefined));
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).toContain("正在读取命令状态");
  expect(wrapper.text()).toContain("外观");
  expect(wrapper.text()).toContain("0.0.9");
});

test("shows a command-card error without hiding other settings", async () => {
  api.getDshCliStatus.mockRejectedValue({ message: "cli status failed" });
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).toContain("无法读取命令行状态");
  expect(wrapper.text()).toContain("cli status failed");
  expect(wrapper.text()).toContain("外观");
  expect(wrapper.text()).toContain("0.0.9");
});
