// 升级状态管理。对应设计 §M3.5 + §5.4（PR-018 Node 升级流程）。
// 跟踪升级检查、安装、pending 状态；支持后台事件通知。

import { defineStore } from "pinia";
import { ref } from "vue";
import {
  checkForUpgrade,
  prepareUpgrade,
  type NodeBlockInfo,
  type UpgradeCheckResult,
} from "@/lib/tauri";

export const useUpgradeStore = defineStore("upgrade", () => {
  /** 是否有可用升级。 */
  const available = ref(false);

  /** 可升级到的版本号。 */
  const version = ref<string | null>(null);

  /** 是否正在检查。 */
  const checking = ref(false);

  /** 是否正在安装升级。 */
  const upgrading = ref(false);

  /** 是否显示升级对话框（重启提示）。 */
  const showDialog = ref(false);

  /** 检查错误信息。 */
  const error = ref<string | null>(null);

  /** Node 版本阻塞（PR-018）：新版 dsh 需要更高 Node。非空时 UI 显示 Node 升级流程。 */
  const nodeBlock = ref<NodeBlockInfo | null>(null);

  /** 是否正在升级 Node（PR-018）。 */
  const upgradingNode = ref(false);

  const emptyResult: UpgradeCheckResult = {
    available: false,
    version: null,
    engines_node: null,
    node_block: null,
  };

  /** 检查升级。不修改 state，仅返回是否有可用版本。 */
  async function check(): Promise<UpgradeCheckResult> {
    if (checking.value) {
      return emptyResult;
    }
    checking.value = true;
    error.value = null;
    try {
      const result = await checkForUpgrade();
      available.value = result.available;
      version.value = result.version;
      nodeBlock.value = result.node_block;
      return result;
    } catch (e) {
      error.value =
        typeof e === "object" && e !== null && "message" in e
          ? (e as { message: string }).message
          : String(e);
      available.value = false;
      version.value = null;
      nodeBlock.value = null;
      return emptyResult;
    } finally {
      checking.value = false;
    }
  }

  /** 安装升级：下载 + npm install + 设 pending。 */
  async function prepare(): Promise<string | null> {
    if (upgrading.value) return null;
    upgrading.value = true;
    error.value = null;
    try {
      const v = await prepareUpgrade();
      version.value = v;
      showDialog.value = true;
      return v;
    } catch (e) {
      error.value =
        typeof e === "object" && e !== null && "message" in e
          ? (e as { message: string }).message
          : String(e);
      return null;
    } finally {
      upgrading.value = false;
    }
  }

  /** 清除 Node 升级阻塞状态（用户取消时）。 */
  function clearNodeBlock(): void {
    nodeBlock.value = null;
  }

  /** 关闭升级对话框。 */
  function dismissDialog(): void {
    showDialog.value = false;
  }

  /** 重置状态。 */
  function reset(): void {
    available.value = false;
    version.value = null;
    checking.value = false;
    upgrading.value = false;
    showDialog.value = false;
    error.value = null;
    nodeBlock.value = null;
    upgradingNode.value = false;
  }

  return {
    available,
    version,
    checking,
    upgrading,
    showDialog,
    error,
    nodeBlock,
    upgradingNode,
    check,
    prepare,
    clearNodeBlock,
    dismissDialog,
    reset,
  };
});
