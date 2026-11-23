<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Type } from "lucide-vue-next";

const storageKey = "font-loader-interface-fonts";
const dialog = ref<HTMLDialogElement>();
const primary = ref("");
const fallbacks = ref("");
let localFaces: FontFace[] = [];

function loadFonts() {
  const savedFonts = localStorage.getItem(storageKey) ?? "";
  const [savedPrimary = "", ...savedFallbacks] = savedFonts.split("\n");
  primary.value = savedPrimary;
  fallbacks.value = savedFallbacks.join("\n");
}

const quoteFamily = (family: string) => `"${CSS.escape(family)}"`;

function applyFonts() {
  const primaryFamily = primary.value.trim();
  const fallbackFamilies = fallbacks.value
    .split("\n")
    .map(family => family.trim())
    .filter(Boolean);
  const families = [primaryFamily, ...fallbackFamilies];
  localFaces.forEach(face => document.fonts.delete(face));
  localFaces = [];
  const [primarySources, ...fallbackSources] = families.map((family, index) => {
    if (!family) return "";
    const name = quoteFamily(family);
    const alias = `font-loader-local-face-${index}`;
    const face = new FontFace(alias, `local(${name})`);
    document.fonts.add(face);
    localFaces.push(face);
    return `${name}, ${quoteFamily(alias)}`;
  });

  const style = document.documentElement.style;
  style.setProperty("--interface-primary-font", primarySources);
  style.setProperty(
    "--interface-fallback-fonts",
    fallbackSources.length ? `${fallbackSources.join(", ")},` : "",
  );
  return families.join("\n");
}

function openSettings() {
  loadFonts();
  dialog.value?.showModal();
}

function resetFonts() {
  primary.value = "";
  fallbacks.value = "";
}

function saveFonts() {
  localStorage.setItem(storageKey, applyFonts());
  dialog.value?.close();
}

onMounted(() => {
  loadFonts();
  applyFonts();
});
</script>

<template>
  <button
    class="icon-button interface-font-toggle"
    type="button"
    aria-label="界面字体设置"
    title="界面字体设置"
    aria-haspopup="dialog"
    @click="openSettings"
  >
    <Type :size="17" aria-hidden="true" />
  </button>

  <Teleport to="body">
    <dialog ref="dialog" class="interface-font-dialog" aria-labelledby="interface-font-title">
      <form @submit.prevent="saveFonts">
        <h2 id="interface-font-title">界面字体</h2>

        <div class="interface-font-field">
          <label for="interface-primary-font">主字体</label>
          <input
            id="interface-primary-font"
            v-model="primary"
            type="text"
            placeholder="Segoe UI"
            autofocus
          />
        </div>

        <div class="interface-font-field">
          <label for="interface-fallback-fonts">回退字体</label>
          <textarea
            id="interface-fallback-fonts"
            v-model="fallbacks"
            rows="4"
            aria-describedby="interface-fallback-help"
          ></textarea>
          <p id="interface-fallback-help">输入字体族名称、字体全名或 PostScript 名，每行一个，按从上到下的顺序尝试。</p>
        </div>

        <div class="dialog-actions">
          <button class="button button-secondary reset-fonts" type="button" @click="resetFonts">
            恢复默认
          </button>
          <button class="button button-secondary" type="button" @click="dialog?.close()">
            取消
          </button>
          <button class="button button-primary" type="submit">保存</button>
        </div>
      </form>
    </dialog>
  </Teleport>
</template>

<style scoped>
.interface-font-toggle {
  flex: 0 0 30px;
}

.interface-font-dialog {
  width: min(440px, calc(100vw - 48px));
  max-height: calc(100vh - 48px);
  padding: 20px;
  border: 1px solid var(--border-strong);
  border-radius: 8px;
  color: var(--text);
  background: var(--surface);
  box-shadow: 0 18px 50px rgb(0 0 0 / 24%);
}

.interface-font-dialog::backdrop {
  background: rgb(18 22 27 / 46%);
}

.interface-font-dialog h2 {
  font-size: 17px;
}

.interface-font-field {
  display: grid;
  gap: 6px;
  margin-top: 14px;
}

.interface-font-field input,
.interface-font-field textarea {
  width: 100%;
  padding: 8px;
  border: 1px solid var(--border-strong);
  border-radius: 6px;
  color: var(--text);
  background: var(--surface);
}

.interface-font-field textarea {
  resize: vertical;
}

.interface-font-field p {
  color: var(--text-muted);
  font-size: 12px;
}

.reset-fonts {
  margin-right: auto;
}
</style>
