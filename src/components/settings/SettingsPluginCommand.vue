<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import {
  CheckCircle2,
  CircleAlert,
  Download,
  LoaderCircle,
  ShieldCheck,
  Terminal,
  Trash2,
} from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useI18n } from "@/lib/i18n";
import {
  runPluginCommand,
  type LauncherErrorPayload,
  type PluginCommandResult,
} from "@/lib/tauri";

const command = ref("");
const input = ref<InstanceType<typeof Input> | null>(null);
const preview = ref<ParsedPluginCommand | null>(null);
const validationError = ref<string | null>(null);
const result = ref<PluginCommandResult | null>(null);
const operationError = ref<string | null>(null);
const running = ref(false);
const { t } = useI18n();

type PluginAction = "add" | "remove";

interface ParsedPluginCommand {
  action: PluginAction;
  profile: string;
  source: string;
}

const actionLabel = computed(() =>
  preview.value?.action === "remove"
    ? t("pluginCommand.remove")
    : t("pluginCommand.install"),
);

function parseCommand(value: string): ParsedPluginCommand | null {
  const parts = value.trim().split(/\s+/);
  if (parts.length !== 6) return null;
  const [dsh, plugin, profileFlag, profile, action, source] = parts;
  if (
    dsh !== "dsh" ||
    plugin !== "plugin" ||
    profileFlag !== "--profile" ||
    !/^[A-Za-z0-9_-]{1,64}$/.test(profile) ||
    (action !== "add" && action !== "remove") ||
    !source ||
    source.length > 1024 ||
    /['"\u0000-\u001F\u007F]/.test(source)
  ) {
    return null;
  }
  return { action, profile, source };
}

function updateCommand(action: PluginAction): void {
  command.value = `dsh plugin --profile web ${action} `;
  preview.value = null;
  result.value = null;
  validationError.value = null;
  operationError.value = null;
  void nextTick(() => input.value?.$el.focus());
}

function prepareCommand(): void {
  result.value = null;
  operationError.value = null;
  const parsed = parseCommand(command.value);
  if (!parsed) {
    preview.value = null;
    validationError.value = t("pluginCommand.invalid");
    return;
  }
  validationError.value = null;
  preview.value = parsed;
}

function editCommand(): void {
  preview.value = null;
  operationError.value = null;
  void nextTick(() => input.value?.$el.focus());
}

async function executeCommand(): Promise<void> {
  if (!preview.value || running.value) return;
  running.value = true;
  operationError.value = null;
  try {
    result.value = await runPluginCommand(command.value);
    preview.value = null;
  } catch (error) {
    const payload = error as LauncherErrorPayload;
    operationError.value =
      payload.user_message ?? payload.message ?? t("pluginCommand.failed");
  } finally {
    running.value = false;
  }
}
</script>

<template>
  <section
    class="settings-plugin-command"
    :aria-label="t('pluginCommand.title')"
  >
    <div class="settings-plugin-command-shell">
      <header class="settings-plugin-command-heading">
        <div class="settings-plugin-command-mark" aria-hidden="true">
          <Terminal />
        </div>
        <div>
          <div class="mb-2 flex items-center gap-2">
            <h1>{{ t("pluginCommand.title") }}</h1>
            <Badge variant="outline">{{ t("pluginCommand.managed") }}</Badge>
          </div>
          <p>{{ t("pluginCommand.description") }}</p>
        </div>
      </header>

      <Card class="settings-plugin-command-card">
        <CardHeader class="gap-3 pb-4">
          <div class="flex items-center justify-between gap-3">
            <CardTitle class="text-base">{{
              t("pluginCommand.commandTitle")
            }}</CardTitle>
            <span class="text-xs text-muted-foreground">{{
              t("pluginCommand.supported")
            }}</span>
          </div>
          <div class="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="secondary"
              size="xs"
              @click="updateCommand('add')"
            >
              <Download />{{ t("pluginCommand.quickInstall") }}
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="xs"
              @click="updateCommand('remove')"
            >
              <Trash2 />{{ t("pluginCommand.quickRemove") }}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <form class="space-y-3" @submit.prevent="prepareCommand">
            <div class="settings-plugin-command-input-wrap">
              <span class="settings-plugin-command-prompt" aria-hidden="true"
                >›</span
              >
              <Input
                ref="input"
                v-model="command"
                class="settings-plugin-command-input font-mono text-sm"
                :aria-label="t('pluginCommand.inputLabel')"
                autocomplete="off"
                autocapitalize="off"
                spellcheck="false"
                placeholder="dsh plugin --profile web add <source>"
                @input="
                  preview = null;
                  result = null;
                  validationError = null;
                  operationError = null;
                "
              />
            </div>
            <div class="flex items-center justify-between gap-3">
              <p class="text-xs text-muted-foreground">
                {{ t("pluginCommand.hint") }}
              </p>
              <Button
                type="submit"
                size="sm"
                :disabled="running || !command.trim()"
              >
                {{ t("pluginCommand.review") }}
              </Button>
            </div>
          </form>

          <p
            v-if="validationError"
            class="settings-plugin-command-feedback text-destructive"
            role="alert"
          >
            <CircleAlert aria-hidden="true" />{{ validationError }}
          </p>

          <div v-if="preview" class="settings-plugin-command-review">
            <div class="flex items-start gap-3">
              <ShieldCheck
                class="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground"
                aria-hidden="true"
              />
              <div class="min-w-0">
                <p class="text-sm font-medium">
                  {{ t("pluginCommand.reviewTitle", { action: actionLabel }) }}
                </p>
                <p class="mt-1 text-xs text-muted-foreground">
                  {{ t("pluginCommand.reviewDescription") }}
                </p>
              </div>
            </div>
            <dl class="settings-plugin-command-details">
              <div>
                <dt>{{ t("pluginCommand.action") }}</dt>
                <dd>{{ actionLabel }}</dd>
              </div>
              <div>
                <dt>{{ t("pluginCommand.profile") }}</dt>
                <dd class="font-mono">{{ preview.profile }}</dd>
              </div>
              <div>
                <dt>{{ t("pluginCommand.source") }}</dt>
                <dd class="break-all font-mono">{{ preview.source }}</dd>
              </div>
            </dl>
            <div class="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                :disabled="running"
                @click="editCommand"
                >{{ t("pluginCommand.edit") }}</Button
              >
              <Button
                type="button"
                :variant="
                  preview.action === 'remove' ? 'destructive' : 'default'
                "
                size="sm"
                :disabled="running"
                @click="executeCommand"
              >
                <LoaderCircle v-if="running" class="animate-spin" />
                <Trash2 v-else-if="preview.action === 'remove'" />
                <Download v-else />
                {{
                  running
                    ? t("pluginCommand.running")
                    : t("pluginCommand.confirm", { action: actionLabel })
                }}
              </Button>
            </div>
          </div>

          <div
            v-if="result"
            class="settings-plugin-command-success"
            role="status"
          >
            <CheckCircle2 aria-hidden="true" />
            <div>
              <p class="text-sm font-medium">
                {{
                  t("pluginCommand.completed", {
                    action:
                      result.action === "remove"
                        ? t("pluginCommand.remove")
                        : t("pluginCommand.install"),
                  })
                }}
              </p>
              <p
                v-if="result.summary !== '操作已完成。'"
                class="mt-1 text-xs text-muted-foreground"
              >
                {{ result.summary }}
              </p>
            </div>
          </div>
          <p
            v-if="operationError"
            class="settings-plugin-command-feedback text-destructive"
            role="alert"
          >
            <CircleAlert aria-hidden="true" />{{ operationError }}
          </p>
        </CardContent>
      </Card>

      <footer class="settings-plugin-command-note">
        <ShieldCheck aria-hidden="true" />
        <p>{{ t("pluginCommand.safety") }}</p>
      </footer>
    </div>
  </section>
</template>

<style scoped>
.settings-plugin-command {
  min-height: 0;
  flex: 1;
  overflow: auto;
  padding: clamp(32px, 8vh, 88px) 32px 48px;
}

.settings-plugin-command-shell {
  width: min(720px, 100%);
  margin: 0 auto;
}

.settings-plugin-command-heading {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  margin-bottom: 32px;
}

.settings-plugin-command-heading h1 {
  font-size: 1.25rem;
  font-weight: 650;
  letter-spacing: -0.015em;
}

.settings-plugin-command-heading p {
  max-width: 56ch;
  font-size: 0.875rem;
  line-height: 1.55;
  color: hsl(var(--muted-foreground));
}

.settings-plugin-command-mark {
  display: grid;
  width: 38px;
  height: 38px;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid hsl(var(--border));
  border-radius: 10px;
  background: hsl(var(--muted) / 0.45);
}

.settings-plugin-command-mark :deep(svg) {
  width: 18px;
  height: 18px;
}

.settings-plugin-command-card {
  box-shadow: 0 14px 30px -24px hsl(var(--foreground) / 0.42);
}

.settings-plugin-command-input-wrap {
  display: flex;
  align-items: center;
  border: 1px solid hsl(var(--input));
  border-radius: 0.5rem;
  background: hsl(var(--muted) / 0.3);
  transition:
    border-color 160ms ease-out,
    box-shadow 160ms ease-out;
}

.settings-plugin-command-input-wrap:focus-within {
  border-color: hsl(var(--ring));
  box-shadow: 0 0 0 2px hsl(var(--ring) / 0.16);
}

.settings-plugin-command-prompt {
  padding-left: 13px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  color: hsl(var(--muted-foreground));
}

.settings-plugin-command-input {
  height: 44px;
  border: 0;
  background: transparent;
  box-shadow: none;
}

.settings-plugin-command-input:focus-visible {
  box-shadow: none;
}

.settings-plugin-command-feedback,
.settings-plugin-command-success {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-top: 14px;
  font-size: 0.8125rem;
  line-height: 1.45;
}

.settings-plugin-command-feedback :deep(svg),
.settings-plugin-command-success :deep(svg),
.settings-plugin-command-note :deep(svg) {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  margin-top: 1px;
}

.settings-plugin-command-review {
  margin-top: 18px;
  padding: 16px;
  border: 1px solid hsl(var(--border));
  border-radius: 0.6rem;
  background: hsl(var(--muted) / 0.28);
}

.settings-plugin-command-details {
  display: grid;
  gap: 8px;
  margin: 16px 0;
  padding: 12px 0;
  border-top: 1px solid hsl(var(--border));
  border-bottom: 1px solid hsl(var(--border));
}

.settings-plugin-command-details div {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 12px;
  font-size: 0.8125rem;
  line-height: 1.45;
}

.settings-plugin-command-details dt {
  color: hsl(var(--muted-foreground));
}

.settings-plugin-command-success {
  margin-top: 18px;
  padding: 14px;
  border: 1px solid hsl(var(--success) / 0.32);
  border-radius: 0.6rem;
  background: hsl(var(--success) / 0.08);
}

.settings-plugin-command-success :deep(svg) {
  color: hsl(var(--success));
}

.settings-plugin-command-note {
  display: flex;
  gap: 8px;
  margin: 18px 4px 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
  line-height: 1.5;
}

@media (max-width: 640px) {
  .settings-plugin-command {
    padding: 28px 18px 36px;
  }

  .settings-plugin-command-heading {
    margin-bottom: 24px;
  }
}
</style>
