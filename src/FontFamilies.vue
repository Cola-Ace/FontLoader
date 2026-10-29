<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { Check, ChevronDown, ChevronUp, Copy } from "lucide-vue-next";
import type { FontEntry } from "./types";

const props = defineProps<{ font: FontEntry; searchTerm: string }>();
const expanded = ref(false);
const allFamiliesKey = Symbol("all-families");
const copiedKey = ref<string | symbol | null>(null);
const copyMessage = ref("");
const copyFailed = ref(false);
const copying = ref(false);
let copyTimer: number | undefined;

const families = computed(() => {
  const groups = new Map<string, Set<string>>();
  for (const face of props.font.faces) {
    if (!groups.has(face.family)) {
      groups.set(face.family, new Set());
    }
    groups.get(face.family)!.add(face.style);
  }
  return [...groups].map(([name, styles]) => ({ name, styles: [...styles] }));
});
const showAll = computed(() => expanded.value || !!props.searchTerm.trim());
const visibleFamilies = computed(() =>
  showAll.value ? families.value : families.value.slice(0, 3),
);

async function copyFamilies(names: string[], key: string | symbol) {
  window.clearTimeout(copyTimer);
  copying.value = true;
  copiedKey.value = null;
  copyMessage.value = "";
  copyFailed.value = false;
  try {
    await navigator.clipboard.writeText(names.join("\n"));
    copiedKey.value = key;
    copyMessage.value = names.length === 1
      ? `已复制：${names[0]}`
      : `已复制 ${names.length} 个 family name`;
    copyTimer = window.setTimeout(() => {
      copiedKey.value = null;
      copyMessage.value = "";
    }, 2000);
  } catch {
    copyFailed.value = true;
    copyMessage.value = "复制失败，请选择名称后按 Ctrl+C";
  } finally {
    copying.value = false;
  }
}

onUnmounted(() => window.clearTimeout(copyTimer));
</script>

<template>
  <div class="font-identity">
    <div class="font-file-header">
      <span class="file-name" :title="font.canonicalPath">{{ font.fileName }}</span>
      <span class="format-badge">{{ font.format }}</span>
      <span v-if="font.format === 'TTC'" class="collection-count">
        {{ families.length }} 个字体族 · {{ font.faces.length }} 个字体面
      </span>
      <button
        v-if="families.length > 1"
        class="text-button copy-all"
        type="button"
        :disabled="copying"
        :aria-label="`复制 ${font.fileName} 的全部 family name`"
        @click="copyFamilies(families.map(family => family.name), allFamiliesKey)"
      >
        <Check v-if="copiedKey === allFamiliesKey" :size="14" />
        <Copy v-else :size="14" />
        {{ copiedKey === allFamiliesKey ? "已复制" : "复制全部" }}
      </button>
    </div>
    <ul class="family-list">
      <li v-for="family in visibleFamilies" :key="family.name" class="family-row">
        <div class="family-name-line">
          <strong class="family-name">{{ family.name }}</strong>
          <button
            class="icon-button copy-family"
            :class="{ 'is-copied': copiedKey === family.name }"
            type="button"
            :disabled="copying"
            :aria-label="`复制 family name：${family.name}`"
            :title="copiedKey === family.name ? '已复制' : '复制 family name'"
            @click="copyFamilies([family.name], family.name)"
          >
            <Check v-if="copiedKey === family.name" :size="15" />
            <Copy v-else :size="15" />
          </button>
        </div>
        <span class="family-styles">{{ family.styles.join(" · ") }}</span>
      </li>
    </ul>
    <button
      v-if="families.length > 3 && !searchTerm.trim()"
      class="text-button collection-toggle"
      type="button"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <ChevronUp v-if="expanded" :size="14" />
      <ChevronDown v-else :size="14" />
      {{ expanded ? "收起" : `展开其余 ${families.length - 3} 个字体族` }}
    </button>
    <span class="copy-feedback" :class="{ 'visually-hidden': !copyFailed }" role="status">
      {{ copyMessage }}
    </span>
  </div>
</template>
