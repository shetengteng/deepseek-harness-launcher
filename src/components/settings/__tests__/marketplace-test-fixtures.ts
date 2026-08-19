import { createPinia, setActivePinia } from "pinia";
import { mount } from "@vue/test-utils";
import SettingsMarketplace from "@/components/settings/SettingsMarketplace.vue";

export const snapshot = {
  source: {
    label: "awesome-dsh-plugin.com",
    url: "https://awesome-dsh-plugin.com/plugins.json",
    fetched_at: "2026-08-18T00:00:00Z",
    catalog_updated_at: "2026-08-18T00:00:00Z",
    catalog_count: 1,
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
        source: "github:owner/plugin",
      },
      description: "A plugin for testing.",
      category: "开发工具",
      category_id: "development",
      tags: ["测试"],
      source_updated_at: "2026-08-17T00:00:00Z",
      validated_at: null,
      popularity: {
        marketplace_rank: null,
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

export function mountMarketplace() {
  const pinia = createPinia();
  setActivePinia(pinia);
  return mount(SettingsMarketplace, { global: { plugins: [pinia], stubs } });
}
