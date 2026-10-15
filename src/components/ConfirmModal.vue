<script setup lang="ts">
import { t } from "../composables/useI18n";
import { onMounted, onUnmounted } from "vue";
import { useConfirm } from "../composables/useConfirm";

const { state, accept, cancel } = useConfirm();

function onBackdrop(ev: MouseEvent) {
  if (ev.target === ev.currentTarget) cancel();
}

function onKey(ev: KeyboardEvent) {
  if (!state.open) return;
  if (ev.key === "Escape") {
    ev.preventDefault();
    ev.stopPropagation();
    cancel();
  } else if (ev.key === "Enter") {
    ev.preventDefault();
    ev.stopPropagation();
    accept();
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKey, true);
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKey, true);
});
</script>

<template>
  <div v-if="state.open" class="backdrop" @mousedown="onBackdrop">
    <div class="modal" @mousedown.stop>
      <div class="message">{{ state.options?.message }}</div>
      <label v-if="state.options?.rememberKey" class="remember">
        <input type="checkbox" v-model="state.dontAsk" />
        <span>{{ t("Don't ask again") }}</span>
      </label>
      <div class="actions">
        <button class="btn" @click="cancel">
          {{ state.options?.cancelLabel ?? t("Cancel") }}
        </button>
        <button class="btn primary" @click="accept">
          {{ state.options?.confirmLabel ?? t("Confirm") }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1100;
}
.modal {
  min-width: 360px;
  max-width: 480px;
  background: #1e1e1e;
  border: 1px solid #111;
  border-radius: 6px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.7);
  color: #d4d4d4;
  font-size: 13px;
  padding: 18px 18px 14px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.message {
  white-space: pre-wrap;
  line-height: 1.5;
}
.remember {
  display: flex;
  align-items: center;
  gap: 6px;
  color: #aaa;
  font-size: 12px;
  cursor: pointer;
  user-select: none;
}
.remember input {
  cursor: pointer;
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.btn {
  background: #2a2a2a;
  color: #d4d4d4;
  border: 1px solid #333;
  padding: 5px 14px;
  cursor: pointer;
  border-radius: 3px;
  font-size: 12px;
  min-width: 72px;
}
.btn:hover { background: #333; }
.btn.primary {
  background: var(--accent);
  color: var(--accent-on);
  border-color: var(--accent);
  font-weight: 600;
}
.btn.primary:hover {
  background: var(--accent-strong);
  border-color: var(--accent-strong);
}
</style>
