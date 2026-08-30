import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import SettingsEnvironmentCard from "@/components/settings/SettingsEnvironmentCard.vue";

const dshState = {
  current: "0.0.9",
  known_good: null,
  installed: [],
  node_mirror: "https://nodejs.org/dist",
  registry: "https://registry.npmjs.org",
};

test("shows node and host while runtime state is still loading", () => {
  const wrapper = mount(SettingsEnvironmentCard, {
    props: {
      dshState: null,
      stateLoading: true,
      stateError: null,
      nodeVersion: "22.19.0",
      hostOrigin: "http://127.0.0.1:51842/",
      latestVersion: null,
      refreshing: true,
      upgrading: false,
      error: null,
      nodeUpdateLoading: false,
      nodeUpdateError: null,
    },
  });

  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "正在检查更新",
  );
  expect(wrapper.text()).toContain("22.19.0");
  expect(wrapper.text()).toContain("127.0.0.1:51842");
});

test("keeps the current version visible when the latest check fails", () => {
  const wrapper = mount(SettingsEnvironmentCard, {
    props: {
      dshState,
      stateLoading: false,
      stateError: null,
      nodeVersion: "22.19.0",
      hostOrigin: null,
      latestVersion: null,
      refreshing: false,
      upgrading: false,
      error: "无法检查最新版本：npm registry unreachable",
      nodeUpdateLoading: false,
      nodeUpdateError: null,
    },
  });

  expect(wrapper.text()).toContain("0.0.9");
  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "无法检查最新版本",
  );
  expect(wrapper.text()).toContain("尚未运行");
});

test("does not claim the runtime is up to date before the latest version arrives", () => {
  const wrapper = mount(SettingsEnvironmentCard, {
    props: {
      dshState,
      stateLoading: false,
      stateError: null,
      nodeVersion: null,
      hostOrigin: null,
      latestVersion: null,
      refreshing: true,
      upgrading: false,
      error: null,
      nodeUpdateLoading: false,
      nodeUpdateError: null,
    },
  });

  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "正在检查更新",
  );
  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).not.toContain(
    "已是最新版本",
  );
  expect(wrapper.text()).toContain("0.0.9");
});
