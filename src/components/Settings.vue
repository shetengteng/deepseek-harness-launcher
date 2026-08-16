<script setup lang="ts">
// 设置页。对应设计 §M3.5 + §10 + §5.4（PR-018 Node 升级）+ §11.3（PR-019 诊断导出）。
// 展示 dsh 状态、升级策略、已安装版本；支持手动检查更新。

import { onMounted, ref } from "vue";
import {
  RefreshCw,
  Download,
  CheckCircle,
  XCircle,
  Clock,
  RotateCcw,
  ArrowLeft,
  Ban,
  Undo2,
  FileArchive,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  getDshState,
  checkForUpgrade,
  prepareUpgrade,
  installNode,
  setPinnedRange,
  setAutoUpgrade,
  setCheckInterval,
  ignoreVersion,
  unignoreVersion,
  exportDiagnostics,
  type DshStateSnapshot,
  type UpgradeCheckResult,
} from "@/lib/tauri";
import { detectPlatformArch } from "@/stores/launcher";

const emit = defineEmits<{
  (e: "back"): void;
  (e: "upgradeReady", version: string): void;
}>();

const dshState = ref<DshStateSnapshot | null>(null);
const loading = ref(true);
const checking = ref(false);
const upgrading = ref(false);
const upgradeResult = ref<UpgradeCheckResult | null>(null);
const upgradeError = ref<string | null>(null);

/** PR-018：Node 升级流程状态。 */
const upgradingNode = ref(false);
const nodeUpgradeError = ref<string | null>(null);

/** PR-019：诊断导出状态。 */
const exporting = ref(false);
const exportInfo = ref<string | null>(null);

// 本地编辑状态（提交前不回写 state）
const pinnedRangeDraft = ref("");
const checkIntervalDraft = ref(24);

const statusBadgeVariant = (status: string) => {
  switch (status) {
    case "verified":
      return undefined; // default
    case "pending":
      return "secondary" as const;
    case "broken":
      return "destructive" as const;
    default:
      return "outline" as const;
  }
};

const statusIcon = (status: string) => {
  switch (status) {
    case "verified":
      return CheckCircle;
    case "pending":
      return Clock;
    case "broken":
      return XCircle;
    default:
      return Clock;
  }
};

onMounted(async () => {
  await loadDshState();
});

async function loadDshState(): Promise<void> {
  loading.value = true;
  try {
    dshState.value = await getDshState();
    pinnedRangeDraft.value = dshState.value.pinned_range;
    checkIntervalDraft.value = dshState.value.check_interval_hours;
  } catch {
    // 静默失败，UI 会显示加载失败
  } finally {
    loading.value = false;
  }
}

async function handleCheckForUpgrade(): Promise<void> {
  checking.value = true;
  upgradeResult.value = null;
  upgradeError.value = null;
  try {
    upgradeResult.value = await checkForUpgrade();
  } catch (e) {
    upgradeError.value =
      typeof e === "object" && e !== null && "message" in e
        ? (e as { message: string }).message
        : String(e);
  } finally {
    checking.value = false;
  }
}

async function handlePrepareUpgrade(): Promise<void> {
  upgrading.value = true;
  upgradeError.value = null;
  try {
    const version = await prepareUpgrade();
    upgradeResult.value = {
      available: true,
      version,
      engines_node: null,
      node_block: null,
    };
    await loadDshState();
    emit("upgradeReady", version);
  } catch (e) {
    upgradeError.value =
      typeof e === "object" && e !== null && "message" in e
        ? (e as { message: string }).message
        : String(e);
  } finally {
    upgrading.value = false;
  }
}

/** PR-018：升级 Node 后继续 dsh 升级。
 * 流程：installNode(node_target) → prepareUpgrade() → 升级对话框。 */
async function handleUpgradeNodeAndDsh(): Promise<void> {
  const block = upgradeResult.value?.node_block;
  if (!block) return;
  upgradingNode.value = true;
  nodeUpgradeError.value = null;
  try {
    // 1. 安装目标 Node 版本（磁盘/网络错误由后端校验并返回中文提示）
    const { platform, arch } = detectPlatformArch();
    await installNode({
      version: block.node_target,
      mirrorBaseUrl: block.mirror_base_url,
      platform,
      arch,
    });

    // 2. Node 就绪后安装 dsh 升级
    const version = await prepareUpgrade();
    upgradeResult.value = {
      available: true,
      version,
      engines_node: block.engines_node,
      node_block: null,
    };
    await loadDshState();
    emit("upgradeReady", version);
  } catch (e) {
    nodeUpgradeError.value =
      typeof e === "object" && e !== null && "user_message" in e
        ? String(
            (e as { user_message?: unknown }).user_message ??
              (e as { message?: unknown }).message ??
              e,
          )
        : String(e);
  } finally {
    upgradingNode.value = false;
  }
}

/** PR-019：导出诊断信息（state.json + 日志 → zip）。 */
async function handleExportDiagnostics(): Promise<void> {
  exporting.value = true;
  exportInfo.value = null;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const dest = await save({
      title: "导出诊断信息",
      defaultPath: `dsh-launcher-diagnostics-${new Date().toISOString().slice(0, 10)}.zip`,
      filters: [{ name: "ZIP 压缩包", extensions: ["zip"] }],
    });
    if (!dest) return; // 用户取消

    const size = await exportDiagnostics(dest);
    exportInfo.value = `已导出（${(size / 1024).toFixed(1)} KB）：${dest}`;
  } catch (e) {
    exportInfo.value =
      typeof e === "object" && e !== null && "message" in e
        ? (e as { message: string }).message
        : String(e);
  } finally {
    exporting.value = false;
  }
}

async function handleSetPinnedRange(): Promise<void> {
  if (!dshState.value || pinnedRangeDraft.value === dshState.value.pinned_range) return;
  try {
    await setPinnedRange(pinnedRangeDraft.value);
    await loadDshState();
  } catch {
    // 校验失败，恢复原值
    pinnedRangeDraft.value = dshState.value?.pinned_range ?? "~0.1.0";
  }
}

async function handleSetAutoUpgrade(enabled: boolean): Promise<void> {
  try {
    await setAutoUpgrade(enabled);
    if (dshState.value) {
      dshState.value.auto_upgrade = enabled;
    }
  } catch {
    // 静默失败
  }
}

async function handleSetCheckInterval(): Promise<void> {
  if (!dshState.value || checkIntervalDraft.value === dshState.value.check_interval_hours) return;
  try {
    await setCheckInterval(checkIntervalDraft.value);
    await loadDshState();
  } catch {
    checkIntervalDraft.value = dshState.value?.check_interval_hours ?? 24;
  }
}

async function handleIgnoreVersion(version: string): Promise<void> {
  try {
    await ignoreVersion(version);
    upgradeResult.value = null;
    await loadDshState();
  } catch {
    // 静默失败
  }
}

async function handleUnignoreVersion(version: string): Promise<void> {
  try {
    await unignoreVersion(version);
    await loadDshState();
  } catch {
    // 静默失败
  }
}
</script>

<template>
  <div class="flex-1 overflow-auto p-6">
    <div class="max-w-2xl mx-auto space-y-6">
      <!-- 顶部导航 -->
      <div class="flex items-center gap-3">
        <Button variant="ghost" size="icon" @click="emit('back')">
          <ArrowLeft class="h-5 w-5" />
        </Button>
        <h1 class="text-xl font-semibold">设置</h1>
      </div>

      <div v-if="loading" class="text-muted-foreground text-sm">
        加载中…
      </div>

      <div v-else-if="!dshState" class="text-muted-foreground text-sm">
        无法加载设置
      </div>

      <template v-else>
        <!-- 运行时状态 -->
        <Card>
          <CardHeader>
            <CardTitle class="text-base">运行时状态</CardTitle>
          </CardHeader>
          <CardContent class="space-y-3">
            <div class="flex justify-between items-center">
              <span class="text-muted-foreground text-sm">当前版本</span>
              <Badge v-if="dshState.current" variant="default">
                {{ dshState.current }}
              </Badge>
              <span v-else class="text-sm text-muted-foreground">未安装</span>
            </div>
            <div class="flex justify-between items-center">
              <span class="text-muted-foreground text-sm">已知好版本</span>
              <Badge v-if="dshState.known_good" variant="secondary">
                {{ dshState.known_good }}
              </Badge>
              <span v-else class="text-sm text-muted-foreground">无</span>
            </div>
            <div class="flex justify-between items-center">
              <span class="text-muted-foreground text-sm">待生效</span>
              <Badge v-if="dshState.pending" variant="outline">
                {{ dshState.pending }}
              </Badge>
              <span v-else class="text-sm text-muted-foreground">无</span>
            </div>
          </CardContent>
        </Card>

        <!-- 升级策略 -->
        <Card>
          <CardHeader>
            <CardTitle class="text-base">升级策略</CardTitle>
          </CardHeader>
          <CardContent class="space-y-4">
            <div class="space-y-2">
              <Label for="pinned-range">版本范围锁定</Label>
              <Input
                id="pinned-range"
                v-model="pinnedRangeDraft"
                class="max-w-[200px]"
                @blur="handleSetPinnedRange"
              />
              <p class="text-xs text-muted-foreground">
                只接受此 semver 范围内的新版本
              </p>
            </div>

            <div class="flex items-center justify-between">
              <div class="space-y-0.5">
                <Label>自动升级</Label>
                <p class="text-xs text-muted-foreground">
                  新版本就绪后自动重启生效
                </p>
              </div>
              <Switch
                :model-value="dshState.auto_upgrade"
                @update:model-value="handleSetAutoUpgrade"
              />
            </div>

            <div class="space-y-2">
              <Label for="check-interval">检查间隔（小时）</Label>
              <Input
                id="check-interval"
                v-model.number="checkIntervalDraft"
                type="number"
                min="1"
                max="720"
                class="max-w-[100px]"
                @blur="handleSetCheckInterval"
              />
            </div>
          </CardContent>
        </Card>

        <!-- 升级操作 -->
        <Card>
          <CardHeader>
            <CardTitle class="text-base">升级</CardTitle>
          </CardHeader>
          <CardContent class="space-y-4">
            <div class="flex gap-3 flex-wrap">
              <Button
                variant="outline"
                :disabled="checking"
                @click="handleCheckForUpgrade"
              >
                <RefreshCw
                  :class="['h-4 w-4 mr-2', checking && 'animate-spin']"
                />
                {{ checking ? "检查中…" : "检查更新" }}
              </Button>

              <Button
                v-if="upgradeResult?.available && !dshState.pending"
                :disabled="upgrading"
                @click="handlePrepareUpgrade"
              >
                <Download class="h-4 w-4 mr-2" />
                {{ upgrading ? "安装中…" : `安装 ${upgradeResult.version}` }}
              </Button>

              <Button
                v-if="dshState.pending"
                variant="secondary"
                disabled
              >
                <RotateCcw class="h-4 w-4 mr-2" />
                重启以生效 {{ dshState.pending }}
              </Button>
            </div>

            <!-- 升级结果 -->
            <div
              v-if="upgradeResult && !upgradeResult.available && !upgradeResult.node_block"
              class="text-sm text-muted-foreground"
            >
              已是最新版本
            </div>

            <!-- PR-018：Node 版本阻塞提示 + 升级 Node 流程 -->
            <div
              v-if="upgradeResult?.node_block"
              class="rounded border border-amber-500/50 bg-amber-500/5 p-3 space-y-2"
            >
              <div class="text-sm">
                新版本
                <span class="font-mono font-semibold">{{ upgradeResult.node_block.dsh_version }}</span>
                需要 Node
                <span class="font-mono font-semibold">{{ upgradeResult.node_block.engines_node }}</span>
                <template v-if="upgradeResult.node_block.current_node">
                  （当前 {{ upgradeResult.node_block.current_node }}）
                </template>
                ，需先升级 Node 到
                <span class="font-mono font-semibold">{{ upgradeResult.node_block.node_target }}</span>。
              </div>
              <Button
                size="sm"
                :disabled="upgradingNode"
                @click="handleUpgradeNodeAndDsh"
              >
                <Download class="h-4 w-4 mr-2" />
                {{ upgradingNode ? "升级 Node 中…" : `升级 Node 到 ${upgradeResult.node_block.node_target} 并安装` }}
              </Button>
              <div v-if="nodeUpgradeError" class="text-sm text-destructive">
                {{ nodeUpgradeError }}
              </div>
            </div>

            <div
              v-if="upgradeResult?.available && upgradeResult.version"
              class="flex items-center gap-2"
            >
              <span class="text-sm text-muted-foreground">
                {{ upgradeResult.version }} 可用
              </span>
              <Button
                variant="ghost"
                size="sm"
                @click="handleIgnoreVersion(upgradeResult.version!)"
              >
                <Ban class="h-3 w-3 mr-1" />
                忽略此版本
              </Button>
            </div>

            <div
              v-if="upgradeError"
              class="text-sm text-destructive"
            >
              {{ upgradeError }}
            </div>
          </CardContent>
        </Card>

        <!-- 已安装版本列表 -->
        <Card v-if="dshState.installed.length > 0">
          <CardHeader>
            <CardTitle class="text-base">已安装版本</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="space-y-2">
              <div
                v-for="installed in dshState.installed"
                :key="installed.version"
                class="flex items-center justify-between py-1"
              >
                <div class="flex items-center gap-2">
                  <component
                    :is="statusIcon(installed.status)"
                    class="h-4 w-4"
                    :class="{
                      'text-green-600': installed.status === 'verified',
                      'text-yellow-600': installed.status === 'pending',
                      'text-destructive': installed.status === 'broken',
                    }"
                  />
                  <span class="text-sm font-mono">{{ installed.version }}</span>
                </div>
                <Badge :variant="statusBadgeVariant(installed.status)">
                  {{ installed.status }}
                </Badge>
              </div>
            </div>
          </CardContent>
        </Card>

        <!-- 已忽略版本 -->
        <Card v-if="dshState.ignored_versions.length > 0">
          <CardHeader>
            <CardTitle class="text-base">已忽略版本</CardTitle>
          </CardHeader>
          <CardContent>
            <div class="flex flex-wrap gap-1">
              <Badge
                v-for="v in dshState.ignored_versions"
                :key="v"
                variant="outline"
                class="flex items-center gap-1 cursor-pointer hover:bg-secondary"
                @click="handleUnignoreVersion(v)"
              >
                <Undo2 class="h-3 w-3" />
                {{ v }}
              </Badge>
            </div>
          </CardContent>
        </Card>

        <!-- PR-019：诊断导出 -->
        <Card>
          <CardHeader>
            <CardTitle class="text-base">诊断</CardTitle>
          </CardHeader>
          <CardContent class="space-y-2">
            <p class="text-xs text-muted-foreground">
              把应用状态（state.json）与壳子/Host 日志打包为 zip，用于问题排查。
            </p>
            <Button
              variant="outline"
              :disabled="exporting"
              @click="handleExportDiagnostics"
            >
              <FileArchive class="h-4 w-4 mr-2" />
              {{ exporting ? "导出中…" : "导出诊断信息" }}
            </Button>
            <div
              v-if="exportInfo"
              class="text-xs break-all text-muted-foreground"
            >
              {{ exportInfo }}
            </div>
          </CardContent>
        </Card>
      </template>
    </div>
  </div>
</template>