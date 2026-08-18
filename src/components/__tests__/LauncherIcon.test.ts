import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import LauncherIcon from "@/components/LauncherIcon.vue";

test("uses theme classes to render the logo dark on the light theme", () => {
  const image = mount(LauncherIcon).get("img");

  expect(image.classes()).toContain("invert");
  expect(image.classes()).toContain("dark:invert-0");
});
