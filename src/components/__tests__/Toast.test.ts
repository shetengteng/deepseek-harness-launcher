import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, expect, test, vi } from "vitest";

import { Toaster, toast, useToast } from "@/components/ui/toast";

afterEach(() => {
  useToast().dismiss();
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
});

test("renders an update notification until the user closes it", async () => {
  vi.useFakeTimers();
  const wrapper = mount(Toaster, { attachTo: document.body });

  toast({
    duration: Number.POSITIVE_INFINITY,
    title: "发现新版本",
    description: "DeepSeek Harness 0.1.0-rc.7 已可用。",
  });
  await nextTick();

  expect(document.body.textContent).toContain("发现新版本");
  const closeButton = document.body.querySelector<HTMLButtonElement>(
    'button[aria-label="关闭通知"]',
  );
  expect(closeButton).not.toBeNull();
  await closeButton!.click();
  await nextTick();

  expect(useToast().toasts.value[0]?.open).toBe(false);
  wrapper.unmount();
});
