import { flushPromises } from "@vue/test-utils";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() => ({
  marketplaceQuery: vi.fn(),
  marketplaceRefresh: vi.fn(),
  marketplaceParseCustomInstall: vi.fn(),
  marketplaceInstall: vi.fn(),
  marketplaceInstallCustom: vi.fn(),
  marketplaceRemove: vi.fn(),
}));

vi.mock("@/lib/tauri", () => api);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

import MarketplaceOperationErrorDialog from "@/components/settings/MarketplaceOperationErrorDialog.vue";
import { mountMarketplace, snapshot } from "./marketplace-test-fixtures";

beforeEach(() => {
  vi.clearAllMocks();
  api.marketplaceQuery.mockResolvedValue(snapshot);
  api.marketplaceRefresh.mockResolvedValue(snapshot);
  api.marketplaceParseCustomInstall.mockResolvedValue({
    profile: "web",
    source: "github:owner/custom",
    dsh_version: "0.1.0",
  });
  api.marketplaceInstall.mockResolvedValue({
    id: "operation-1",
    kind: "install",
    plugin_id: "owner/plugin",
    profile: "web",
    phase: "succeeded",
    message: "已安装",
    log_path: null,
  });
  api.marketplaceInstallCustom.mockResolvedValue({
    id: "operation-2",
    kind: "custom_install",
    plugin_id: "github:owner/custom",
    profile: "web",
    phase: "succeeded",
    message: "已安装",
    log_path: null,
  });
});

afterEach(() => {
  vi.useRealTimers();
});

test("reloads the marketplace after the keyword debounce", async () => {
  vi.useFakeTimers();
  const wrapper = mountMarketplace();
  await flushPromises();
  api.marketplaceQuery.mockClear();

  let resolveSearch: ((value: typeof snapshot) => void) | null = null;
  api.marketplaceQuery.mockImplementation((query: { query?: string }) => {
    if (query.query === "terminal") {
      return new Promise((resolve) => {
        resolveSearch = resolve;
      });
    }
    return Promise.resolve(snapshot);
  });

  await wrapper.get('input[aria-label="搜索插件"]').setValue("terminal");
  expect(api.marketplaceQuery).not.toHaveBeenCalled();

  vi.advanceTimersByTime(250);
  await flushPromises();

  expect(api.marketplaceQuery).toHaveBeenCalledWith({
    query: "terminal",
    category: undefined,
    installedOnly: false,
    sort: "relevance",
    profile: "web",
  });
  expect(wrapper.find('[aria-label="正在加载目录"]').exists()).toBe(true);

  resolveSearch!(snapshot);
  await flushPromises();
});

test("uses the two latest scope tabs and keeps catalog state in the list pane", async () => {
  const wrapper = mountMarketplace();
  await flushPromises();

  expect(wrapper.findAll('[role="tab"]').map((tab) => tab.text())).toEqual([
    "发现",
    "已安装 0",
  ]);
  expect(wrapper.find("h1").exists()).toBe(false);
  expect(wrapper.find('[role="tablist"]').classes()).toContain(
    "marketplace-tabs",
  );
  expect(wrapper.find('[role="tab"]').classes()).toContain(
    "marketplace-tab-trigger",
  );
  expect(wrapper.text()).toContain("目录已同步");
  expect(wrapper.text()).toContain("awesome-dsh-plugin.com");
});

test("does not render a market rank because the curated registry has no rank", async () => {
  api.marketplaceQuery.mockResolvedValue({
    ...snapshot,
    plugins: [
      {
        ...snapshot.plugins[0],
        popularity: {
          ...snapshot.plugins[0].popularity,
          marketplace_rank: null,
        },
      },
    ],
  });
  const wrapper = mountMarketplace();
  await flushPromises();

  expect(wrapper.find(".marketplace-detail-list").text()).not.toContain(
    "市场排名",
  );
});

test("shows Stars, then waits for a second install confirmation", async () => {
  const wrapper = mountMarketplace();
  await flushPromises();

  expect(wrapper.text()).toContain("312 Stars");

  await wrapper
    .findAll("button")
    .find((button) => button.text() === "安装")!
    .trigger("click");

  expect(wrapper.text()).toContain("确认安装");
  expect(api.marketplaceInstall).not.toHaveBeenCalled();

  await wrapper
    .findAll("button")
    .find((button) => button.text() === "确认安装")!
    .trigger("click");
  await flushPromises();

  expect(api.marketplaceInstall).toHaveBeenCalledWith({
    pluginId: "owner/plugin",
    profile: "web",
  });
});

test("shows installation failures in a dialog with a retry action", async () => {
  const installedSnapshot = {
    ...snapshot,
    plugins: [
      {
        ...snapshot.plugins[0],
        status: "installed" as const,
        installation_id: "dsh-plugin",
        installed_source: "github:owner/plugin",
      },
    ],
  };
  api.marketplaceQuery.mockResolvedValueOnce(snapshot);
  api.marketplaceQuery.mockResolvedValue(installedSnapshot);
  api.marketplaceInstall.mockRejectedValue({
    kind: "marketplace",
    message: "plugin command failed: dsh exited with an error",
    user_message: "插件安装失败，请检查网络后重试。",
  });
  const wrapper = mountMarketplace();
  await flushPromises();

  await wrapper
    .findAll("button")
    .find((button) => button.text() === "安装")!
    .trigger("click");
  await wrapper
    .findAll("button")
    .find((button) => button.text() === "确认安装")!
    .trigger("click");
  await flushPromises();

  const dialog = wrapper.findComponent(MarketplaceOperationErrorDialog);
  expect(dialog.props("error")).toBe("插件安装失败，请检查网络后重试。");
  expect(document.body.textContent).toContain("插件安装失败");
  expect(document.body.textContent).toContain(
    "插件安装失败，请检查网络后重试。",
  );
  expect(document.body.textContent).not.toContain("操作日志");
  expect(wrapper.text()).toContain("已安装");
  const retry = Array.from(document.body.querySelectorAll("button")).find(
    (button) => button.textContent?.trim() === "重试",
  );
  expect(retry).toBeDefined();

  retry!.click();
  await flushPromises();
  expect(api.marketplaceInstall).toHaveBeenCalledTimes(2);

  const close = Array.from(document.body.querySelectorAll("button")).find(
    (button) => button.textContent?.trim() === "关闭",
  );
  close?.click();
});

test("keeps custom installation collapsed until its parsed preview is ready", async () => {
  const wrapper = mountMarketplace();
  await flushPromises();

  expect(
    wrapper
      .find('[aria-controls="custom-install-content"]')
      .attributes("aria-expanded"),
  ).toBe("false");

  await wrapper
    .get('[aria-controls="custom-install-content"]')
    .trigger("click");
  await wrapper
    .get('input[aria-label="自定义插件安装命令"]')
    .setValue("dsh plugin --profile web add github:owner/custom");
  await wrapper.get("#custom-install-content").trigger("submit");
  await flushPromises();

  expect(api.marketplaceParseCustomInstall).toHaveBeenCalledWith(
    "dsh plugin --profile web add github:owner/custom",
  );
  expect(wrapper.text()).toContain("自定义来源，尚未执行");
  expect(api.marketplaceInstallCustom).not.toHaveBeenCalled();

  await wrapper
    .findAll("button")
    .find((button) => button.text() === "确认安装")!
    .trigger("click");
  await flushPromises();

  expect(api.marketplaceInstallCustom).toHaveBeenCalledWith(
    "dsh plugin --profile web add github:owner/custom",
  );
});
