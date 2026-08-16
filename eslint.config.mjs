// ESLint 9 扁平配置。对应测试设计 §0 通用门禁 `pnpm lint`。
// Vue 3 + TypeScript 规则集来自 @vue/eslint-config-typescript v14。
// `defineConfigWithVueTs` 自动集成 eslint-plugin-vue + @typescript-eslint 的 recommended 配置。
import { defineConfigWithVueTs } from '@vue/eslint-config-typescript'

export default defineConfigWithVueTs(
  // 忽略目录：构建产物、依赖、Rust 工程、shadcn-vue 生成的 ui 目录
  {
    ignores: [
      'dist/**',
      'node_modules/**',
      'src-tauri/**',
      'public/**',
      'src/components/ui/**', // shadcn-vue 自管源码，不在本项目 lint 范围
      '*.config.*',
    ],
  },
)
