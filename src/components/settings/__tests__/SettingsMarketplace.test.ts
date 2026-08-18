import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

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

import SettingsMarketplace from "@/components/settings/SettingsMarketplace.vue";

const snapshot = {
  source: {
    label: "DSH 1024Store",
    fetched_at: "2026-08-18T00:00:00Z",
    stale: false,
  },
  profiles: ["web"],
  plugins: [
    {
      id: "owner/plugin",
      name: "dsh-plugin",
      repository_url: "https://github.com/owner/plugin",
      install_spec: {
        owner: "owner",
        repository: "plugin",
        subdirectory: null,
        reference: null,
      },
      description: "A plugin for testing.",
      category: "开发工具",
      tags: ["测试"],
      source_updated_at: "2026-08-17T00:00:00Z",
      validated_at: null,
      popularity: {
        marketplace_rank: 8,
        ranking_updated_at: null,
        github_stars: 312,
        stars_fetched_at: null,
      },
      status: "available" as const,
      installation_id: null,
      installed_source: null,
      local_only: false,
    },
  ],
};

const stubs = {
  Button: {
    emits: ["click"],
    template:
      '<button v-bind="$attrs" :type="$attrs.type" @click="$emit(\'click\')"><slot /></button>',
  },
  Badge: { template: "<span><slot /></span>" },
  Input: {
    props: ["modelValue"],
    emits: ["update:modelValue"],
    template:
      '<input v-bind="$attrs" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  },
  Select: { template: "<div><slot /></div>" },
  SelectContent: { template: "<div><slot /></div>" },
  SelectItem: { template: "<div><slot /></div>" },
  SelectTrigger: { template: "<button><slot /></button>" },
  SelectValue: { template: "<span />" },
};

function mountMarketplace() {
  const pinia = createPinia();
  setActivePinia(pinia);
  return mount(SettingsMarketplace, { global: { plugins: [pinia], stubs } });
}

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

test("shows rank and Stars, then waits for a second install confirmation", async () => {
  const wrapper = mountMarketplace();
  await flushPromises();

  expect(wrapper.text()).toContain("#8");
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

test("keeps custom installation collapsed until its parsed preview is ready", async () => {
  const wrapper = mountMarketplace();
  await flushPromises();

  expect(wrapper.find('[aria-controls="custom-install-content"]').attributes("aria-expanded")).toBe("false");

  await wrapper.get('[aria-controls="custom-install-content"]').trigger("click");
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
