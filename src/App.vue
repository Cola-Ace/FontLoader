<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  CheckCircle2,
  FilePlus2,
  LoaderCircle,
  Moon,
  RotateCcw,
  Search,
  Sun,
  Trash2,
  X,
} from "lucide-vue-next";
import type {
  AppSnapshot,
  BatchProgress,
  BatchResult,
  ExitResult,
  FontEntry,
  OperationError,
} from "./types";

const emptySnapshot = (): AppSnapshot => ({
  fonts: [],
  restoreCandidates: [],
  exitErrors: [],
});

const snapshot = ref<AppSnapshot>(emptySnapshot());
const selectedPaths = ref<Set<string>>(new Set());
const searchTerm = ref("");
const busy = ref(false);
const operationErrors = ref<OperationError[]>([]);
const statusMessage = ref("");
const operationProgress = ref<BatchProgress | null>(null);
const exitErrors = ref<OperationError[]>([]);
const theme = ref<"light" | "dark">("light");
let stopProgressListener: (() => void) | undefined;
let statusDismissTimer: number | undefined;

const filteredFonts = computed(() => {
  const term = searchTerm.value.trim().toLocaleLowerCase();
  if (!term) {
    return snapshot.value.fonts;
  }
  return snapshot.value.fonts.filter(font => {
    const searchable = [
      font.fileName,
      font.canonicalPath,
      font.format,
      ...font.faces.flatMap(face => [face.family, face.style]),
    ];
    return searchable.some(value => value.toLocaleLowerCase().includes(term));
  });
});

const allVisibleSelected = computed(
  () =>
    filteredFonts.value.length > 0 &&
    filteredFonts.value.every(font => selectedPaths.value.has(font.canonicalPath)),
);

const faceLabel = (font: FontEntry) =>
  font.faces.map(face => `${face.family} · ${face.style}`).join("；");

const loadedTime = (value: string) =>
  new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));

function applySnapshot(next: AppSnapshot) {
  snapshot.value = next;
  const currentPaths = new Set(next.fonts.map(font => font.canonicalPath));
  selectedPaths.value = new Set(
    [...selectedPaths.value].filter(path => currentPaths.has(path)),
  );
  exitErrors.value = next.exitErrors;
}

function setSelection(path: string, selected: boolean) {
  const next = new Set(selectedPaths.value);
  if (selected) {
    next.add(path);
  } else {
    next.delete(path);
  }
  selectedPaths.value = next;
}

function toggleAllVisible(selected: boolean) {
  const next = new Set(selectedPaths.value);
  for (const font of filteredFonts.value) {
    if (selected) {
      next.add(font.canonicalPath);
    } else {
      next.delete(font.canonicalPath);
    }
  }
  selectedPaths.value = next;
}

function summarizeResult(result: BatchResult, action: "加载" | "卸载") {
  const messages = [`${action}成功 ${result.changed.length} 个`];
  if (result.skipped.length) {
    messages.push(`跳过 ${result.skipped.length} 个`);
  }
  if (result.errors.length) {
    messages.push(`失败 ${result.errors.length} 个`);
  }
  statusMessage.value = messages.join("，");
  operationErrors.value = result.errors;
  clearStatusTimer();
  statusDismissTimer = window.setTimeout(() => {
    statusMessage.value = "";
    operationErrors.value = [];
    statusDismissTimer = undefined;
  }, 3000);
}

function clearStatusTimer() {
  if (statusDismissTimer !== undefined) {
    window.clearTimeout(statusDismissTimer);
    statusDismissTimer = undefined;
  }
}

function dismissStatus() {
  clearStatusTimer();
  statusMessage.value = "";
  operationErrors.value = [];
}

function applyTheme(nextTheme: "light" | "dark") {
  theme.value = nextTheme;
  document.documentElement.dataset.theme = nextTheme;
  localStorage.setItem("font-loader-theme", nextTheme);
}

function toggleTheme() {
  applyTheme(theme.value === "light" ? "dark" : "light");
}

function operationFor(command: "load_fonts" | "unload_fonts" | "restore_last_session") {
  return command === "unload_fonts" ? "unload" : "load";
}

function totalFor(
  command: "load_fonts" | "unload_fonts" | "restore_last_session",
  args: Record<string, unknown>,
) {
  if (command === "restore_last_session") {
    return snapshot.value.restoreCandidates.length;
  }
  return Array.isArray(args.paths) ? args.paths.length : 0;
}

async function runBatch(
  command: "load_fonts" | "unload_fonts" | "restore_last_session",
  args: Record<string, unknown> = {},
) {
  busy.value = true;
  clearStatusTimer();
  operationErrors.value = [];
  operationProgress.value = {
    operation: operationFor(command),
    completed: 0,
    total: totalFor(command, args),
  };
  statusMessage.value =
    command === "unload_fonts" ? "正在卸载字体..." : "正在加载字体...";
  try {
    const result = await invoke<BatchResult>(command, args);
    applySnapshot(result.snapshot);
    summarizeResult(result, command === "unload_fonts" ? "卸载" : "加载");
  } finally {
    busy.value = false;
    operationProgress.value = null;
  }
}

async function chooseFonts() {
  const chosen = await open({
    multiple: true,
    directory: false,
    filters: [
      {
        name: "字体文件",
        extensions: ["ttf", "otf", "ttc"],
      },
    ],
  });
  if (!chosen) {
    return;
  }
  const paths = Array.isArray(chosen) ? chosen : [chosen];
  await runBatch("load_fonts", { paths });
}

async function unloadSelected() {
  if (!selectedPaths.value.size) {
    return;
  }
  await runBatch("unload_fonts", { paths: [...selectedPaths.value] });
}

async function unloadOne(path: string) {
  await runBatch("unload_fonts", { paths: [path] });
}

async function restoreSession() {
  await runBatch("restore_last_session");
}

async function dismissSession() {
  applySnapshot(await invoke<AppSnapshot>("dismiss_last_session"));
}

async function retryExit(force: boolean) {
  busy.value = true;
  operationProgress.value = {
    operation: "unload",
    completed: 0,
    total: snapshot.value.fonts.length,
  };
  try {
    const result = await invoke<ExitResult>("request_exit", { force });
    exitErrors.value = result.errors;
  } finally {
    busy.value = false;
    operationProgress.value = null;
  }
}

onMounted(async () => {
  const savedTheme = localStorage.getItem("font-loader-theme");
  applyTheme(
    savedTheme === "light" || savedTheme === "dark"
      ? savedTheme
      : window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light",
  );
  if (!("__TAURI_INTERNALS__" in window) && import.meta.env.MODE !== "test") {
    return;
  }
  try {
    stopProgressListener = await listen<BatchProgress>("font-batch-progress", event => {
      if (
        !busy.value ||
        operationProgress.value?.operation !== event.payload.operation
      ) {
        return;
      }
      operationProgress.value = event.payload;
    });
    applySnapshot(await invoke<AppSnapshot>("get_initial_state"));
    await invoke("window_ready");
  } catch (error) {
    operationErrors.value = [
      {
        path: "",
        code: "initialization_failed",
        message: String(error),
      },
    ];
  }
});

onUnmounted(() => {
  stopProgressListener?.();
  clearStatusTimer();
});
</script>

<template>
  <div class="app-shell">
    <header class="app-header">
      <div class="brand-block">
        <h1>FontLoader</h1>
        <span class="font-count">{{ snapshot.fonts.length }} 个已加载</span>
      </div>

      <div class="header-actions">
        <button
          class="icon-button theme-toggle"
          type="button"
          :aria-label="theme === 'light' ? '切换到深色主题' : '切换到浅色主题'"
          :title="theme === 'light' ? '切换到深色主题' : '切换到浅色主题'"
          @click="toggleTheme"
        >
          <Moon v-if="theme === 'light'" :size="17" />
          <Sun v-else :size="17" />
        </button>

        <label class="search-field">
          <Search :size="16" aria-hidden="true" />
          <input v-model="searchTerm" type="search" placeholder="搜索字体" />
          <button
            v-if="searchTerm"
            class="icon-button"
            type="button"
            aria-label="清除搜索"
            title="清除搜索"
            @click="searchTerm = ''"
          >
            <X :size="15" />
          </button>
        </label>

        <button class="button button-primary" type="button" :disabled="busy" @click="chooseFonts">
          <FilePlus2 :size="17" />
          选择字体
        </button>
      </div>
    </header>

    <section v-if="snapshot.restoreCandidates.length" class="restore-band">
      <div>
        <strong>检测到上次会话</strong>
        <span>{{ snapshot.restoreCandidates.length }} 个字体可恢复</span>
      </div>
      <div class="band-actions">
        <button class="button button-secondary" type="button" :disabled="busy" @click="dismissSession">
          忽略
        </button>
        <button class="button button-primary" type="button" :disabled="busy" @click="restoreSession">
          <RotateCcw :size="16" />
          恢复
        </button>
      </div>
    </section>

    <main class="content">
      <div class="list-toolbar">
        <label class="select-all">
          <input
            type="checkbox"
            :checked="allVisibleSelected"
            :disabled="filteredFonts.length === 0"
            aria-label="全选当前列表"
            @change="toggleAllVisible(($event.target as HTMLInputElement).checked)"
          />
          <span>全选</span>
        </label>

        <button
          class="button button-danger-quiet"
          type="button"
          :disabled="busy || selectedPaths.size === 0"
          @click="unloadSelected"
        >
          <Trash2 :size="16" />
          卸载所选（{{ selectedPaths.size }}）
        </button>
      </div>

      <div class="font-table" :class="{ 'is-empty': filteredFonts.length === 0 }">
        <div class="table-head" aria-hidden="true">
          <span></span>
          <span>字体</span>
          <span>格式</span>
          <span>文件</span>
          <span>加载时间</span>
          <span></span>
        </div>

        <div v-if="filteredFonts.length === 0" class="empty-state">
          <span>{{ searchTerm ? "没有匹配的字体" : "尚未加载字体" }}</span>
        </div>

        <div
          v-for="font in filteredFonts"
          :key="font.canonicalPath"
          class="font-row"
          data-testid="font-row"
        >
          <input
            type="checkbox"
            :checked="selectedPaths.has(font.canonicalPath)"
            :aria-label="`选择 ${font.fileName}`"
            @change="setSelection(font.canonicalPath, ($event.target as HTMLInputElement).checked)"
          />
          <div class="font-identity">
            <strong :title="faceLabel(font)">{{ faceLabel(font) }}</strong>
            <span :title="font.canonicalPath">{{ font.canonicalPath }}</span>
          </div>
          <span class="format-badge">{{ font.format }}</span>
          <span class="file-name" :title="font.fileName">{{ font.fileName }}</span>
          <time :datetime="font.loadedAt">{{ loadedTime(font.loadedAt) }}</time>
          <button
            class="icon-button row-action"
            type="button"
            :disabled="busy"
            :aria-label="`卸载 ${font.fileName}`"
            :title="`卸载 ${font.fileName}`"
            @click="unloadOne(font.canonicalPath)"
          >
            <Trash2 :size="16" />
          </button>
        </div>
      </div>

      <section v-if="statusMessage || operationErrors.length" class="operation-status" aria-live="polite">
        <div v-if="statusMessage" class="status-summary">
          <LoaderCircle v-if="busy" class="status-spinner" :size="16" />
          <CheckCircle2 v-else :size="16" />
           <span>
             {{ statusMessage }}
             <strong v-if="busy && operationProgress" class="status-progress">
               {{ operationProgress.completed }}/{{ operationProgress.total }}
             </strong>
           </span>
          <button
            v-if="!busy"
            class="icon-button status-dismiss"
            type="button"
            aria-label="关闭提示"
            title="关闭提示"
            @click="dismissStatus"
          >
            <X :size="14" />
          </button>
        </div>
        <ul v-if="operationErrors.length" class="error-list">
          <li v-for="error in operationErrors" :key="`${error.path}-${error.code}`">
            <AlertTriangle :size="15" />
            <span>
              <strong>{{ error.path || "会话日志" }}</strong>
              {{ error.message }}
            </span>
          </li>
        </ul>
      </section>
    </main>

    <div v-if="exitErrors.length" class="dialog-backdrop" role="presentation">
      <section class="exit-dialog" role="dialog" aria-modal="true" aria-labelledby="exit-dialog-title">
        <AlertTriangle :size="22" />
        <div>
          <h2 id="exit-dialog-title">部分字体卸载失败</h2>
          <ul>
            <li v-for="error in exitErrors" :key="`${error.path}-${error.code}`">
              <strong>{{ error.path }}</strong>
              <span>{{ error.message }}</span>
            </li>
          </ul>
          <div class="dialog-actions">
            <button class="button button-secondary" type="button" :disabled="busy" @click="retryExit(true)">
              仍然退出
            </button>
            <button class="button button-primary" type="button" :disabled="busy" @click="retryExit(false)">
              <RotateCcw :size="16" />
              重试卸载
            </button>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
