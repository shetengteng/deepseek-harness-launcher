#!/usr/bin/env node
// 从 shadcn-vue registry 拉取组件源码并写入 src/components/ui/
// 用法: node scripts/fetch-shadcn.mjs button card dialog ...
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, "..");
const OUT_DIR = resolve(ROOT, "src/components/ui");

const COMPONENTS = process.argv.slice(2);
if (COMPONENTS.length === 0) {
  console.error("Usage: node scripts/fetch-shadcn.mjs button card ...");
  process.exit(1);
}

const REGISTRY = "https://shadcn-vue.com/r/styles/default";

async function fetchComponent(name) {
  const url = `${REGISTRY}/${name}.json`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${name}: HTTP ${res.status}`);
  return res.json();
}

async function main() {
  for (const name of COMPONENTS) {
    const spec = await fetchComponent(name);
    console.log(`→ ${name}: ${spec.files?.length ?? 0} files`);
    if (!spec.files) continue;
    for (const f of spec.files) {
      const target = resolve(OUT_DIR, f.path.replace(/^ui\//, ""));
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, f.content, "utf8");
      console.log(`  ✓ ${f.path}`);
    }
  }
  console.log("done");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
