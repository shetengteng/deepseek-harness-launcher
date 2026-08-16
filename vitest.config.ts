import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import path from "node:path";

// Vitest 配置。对应测试设计 §0 通用门禁 `pnpm test`。
// 复用 vite 的 `@` alias，避免在测试里写相对路径。

export default defineConfig({
  // @vitejs/plugin-vue 的 plugin 类型与 vitest 依赖的 vite 5/6 版本偶有冲突，
  // 用类型断言绕过，运行时无副作用。
  // @ts-expect-error vite plugin 类型版本不匹配
  plugins: [vue()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/__tests__/**/*.test.ts", "src/**/*.test.ts"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      include: ["src/stores/**", "src/components/**", "src/lib/**"],
      exclude: ["src/components/ui/**"],
    },
  },
});
