<script setup lang="ts">
import { computed } from "vue";
import { usePrefs } from "../composables/usePrefs";
import { useWorkspaces } from "../composables/useWorkspaces";
import { useAccountProfiles } from "../composables/useAccountProfiles";
import { t } from "../composables/useI18n";

const props = defineProps<{ sessionId: string }>();
const { prefs } = usePrefs();
const { state } = useWorkspaces();
const { profiles } = useAccountProfiles();
const label = computed(() => {
  if (!prefs.showAccountProfile) return "";
  const profile = state.workspaces.find(ws => ws.terminalSnapshots[props.sessionId])
    ?.terminalSnapshots[props.sessionId]?.accountProfile;
  if (!profile) return "";
  return profile.id === null ? t("System")
    : profiles.find(p => p.id === profile.id)?.label || profile.label;
});
</script>

<template>
  <span v-if="label" class="profile-tag" :title="label">[{{ label }}]</span>
</template>

<style scoped>
.profile-tag {
  flex: 0 1 auto;
  max-width: 120px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #a8c9bd;
  background: rgba(168, 201, 189, 0.08);
  border-radius: 3px;
  padding: 1px 4px;
  font-size: 11px;
  line-height: 16px;
}
</style>
