import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import CrashDialog from "@/components/CrashDialog.vue";

const stubs = {
  Dialog: { template: "<div><slot /></div>" },
  DialogContent: { template: "<div><slot /></div>" },
  DialogDescription: { template: "<div><slot /></div>" },
  DialogFooter: { template: "<div><slot /></div>" },
  DialogHeader: { template: "<div><slot /></div>" },
  DialogTitle: { template: "<div><slot /></div>" },
  Button: {
    emits: ["click"],
    props: ["disabled"],
    template:
      '<button :disabled="disabled" @click="$emit(\'click\')"><slot /></button>',
  },
  AlertTriangle: { template: "<span />" },
  LogOut: { template: "<span />" },
  RotateCcw: { template: "<span />" },
  Undo2: { template: "<span />" },
  X: { template: "<span />" },
};

test("emits exit when the crash dialog exit action is clicked", async () => {
  const wrapper = mount(CrashDialog, {
    props: {
      crash: {
        crash_counter: 3,
        retry_limit: 3,
        exit_code: 17,
        exit_signal: null,
        known_good: "0.2.0",
      },
      recovering: false,
    },
    global: { stubs },
  });

  const exitButton = wrapper
    .findAll("button")
    .find((button) => button.text().includes("退出应用"));
  expect(exitButton).toBeDefined();
  await exitButton!.trigger("click");

  expect(wrapper.emitted("exit")).toHaveLength(1);
});
