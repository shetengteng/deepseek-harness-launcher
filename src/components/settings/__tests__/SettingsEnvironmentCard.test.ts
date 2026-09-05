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

test("reveals the launch token only after the eye toggle", async () => {
  const token = "icFSw9ecAaK8mdgnOsYiKSjSlQFMDxjTCp7EnCZiYZI";
  const wrapper = mount(SettingsEnvironmentCard, {
    props: {
      dshState,
      stateLoading: false,
      stateError: null,
      nodeVersion: "22.19.0",
      hostOrigin: `http://127.0.0.1:51842/?token=${token}`,
      latestVersion: null,
      refreshing: false,
      upgrading: false,
      error: null,
      nodeUpdateLoading: false,
      nodeUpdateError: null,
    },
  });

  const value = wrapper.get('[data-testid="launch-token-value"]');
  expect(value.text()).not.toContain(token);
  expect(value.text()).toContain("•");

  await wrapper.get('[data-testid="launch-token-toggle"]').trigger("click");
  expect(value.text()).toContain(token);

  await wrapper.get('[data-testid="launch-token-toggle"]').trigger("click");
  expect(value.text()).not.toContain(token);
});

test("shows a placeholder instead of the token row when the host is down", () => {
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
      error: null,
      nodeUpdateLoading: false,
      nodeUpdateError: null,
    },
  });

  expect(wrapper.text()).toContain("尚未运行");
  expect(wrapper.find('[data-testid="launch-token-toggle"]').exists()).toBe(
    false,
  );
});

test("marks the token row as tokenless for origins without a token", () => {
  const wrapper = mount(SettingsEnvironmentCard, {
    props: {
      dshState,
      stateLoading: false,
      stateError: null,
      nodeVersion: "22.19.0",
      hostOrigin: "http://127.0.0.1:51842/",
      latestVersion: null,
      refreshing: false,
      upgrading: false,
      error: null,
      nodeUpdateLoading: false,
      nodeUpdateError: null,
    },
  });

  expect(wrapper.text()).toContain("无令牌");
  expect(wrapper.find('[data-testid="launch-token-toggle"]').exists()).toBe(
    false,
  );
});

test("shows a dedicated start-error panel for rollback details", () => {
  const wrapper = mount(SettingsEnvironmentCard, {
    props: {
      dshState,
      stateLoading: false,
      stateError: null,
      nodeVersion: "22.19.0",
      hostOrigin: "http://127.0.0.1:51842/",
      latestVersion: { latest_version: "0.1.0" },
      refreshing: false,
      upgrading: false,
      error: "新版本无法启动，已恢复 0.1.1-rc.2。",
      errorHint: "dsh 启动超时（90 秒内未就绪）。请重试；若持续失败请导出诊断信息。",
      errorTechnical:
        "host supervisor error: desktop Host readiness timed out after 90s",
      nodeUpdateLoading: false,
      nodeUpdateError: null,
    },
  });

  expect(wrapper.get('[data-testid="dsh-update-status"]').text()).toContain(
    "新版本无法启动，已恢复 0.1.1-rc.2。",
  );
  const startError = wrapper.get('[data-testid="dsh-start-error"]');
  expect(startError.text()).toContain("启动失败原因");
  expect(startError.text()).toContain("dsh 启动超时");
  expect(startError.text()).toContain("readiness timed out");
});
