<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { Check, ChevronDown, ChevronUp, Copy } from "lucide-vue-next";
import type { FontEntry } from "./types";

const props = defineProps<{ font: FontEntry; searchTerm: string }>();
const expanded = ref(false);
const copiedKey = ref<string | null>(null);
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
  showAll.value ? families.value : families.value.slice(0, 2),
);

async function copyFamily(name: string) {
  window.clearTimeout(copyTimer);
  copying.value = true;
  copiedKey.value = null;
  copyMessage.value = "";
  copyFailed.value = false;
  try {
    await navigator.clipboard.writeText(name);
    copiedKey.value = name;
    copyMessage.value = `已复制：${name}`;
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
    </div>
    <ul class="family-list">
      <li v-for="family in visibleFamilies" :key="family.name" class="family-row">
        <div class="family-name-line">
          <strong class="family-name">{{ family.name }}</strong>
          <span class="family-styles">{{ family.styles.join(" · ") }}</span>
          <button
            class="icon-button copy-family"
            :class="{ 'is-copied': copiedKey === family.name }"
            type="button"
            :disabled="copying"
            :aria-label="`复制 family name：${family.name}`"
            :title="copiedKey === family.name ? '已复制' : '复制 family name'"
            @click="copyFamily(family.name)"
          >
            <Check v-if="copiedKey === family.name" :size="15" />
            <Copy v-else :size="15" />
          </button>
        </div>
      </li>
    </ul>
    <button
      v-if="families.length > 2 && !searchTerm.trim()"
      class="text-button collection-toggle"
      type="button"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <ChevronUp v-if="expanded" :size="14" />
      <ChevronDown v-else :size="14" />
      {{ expanded ? "收起" : `展开其余 ${families.length - 2} 个字体族` }}
    </button>
    <span class="copy-feedback" :class="{ 'visually-hidden': !copyFailed }" role="status">
      {{ copyMessage }}
    </span>
  </div>
</template>
