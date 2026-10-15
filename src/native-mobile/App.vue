<script setup lang="ts">
import { ref } from "vue";
import icon from "../../src-tauri/icons/128x128.png";
import { gatewayInvitation } from "./invitation";
const invitation = ref("");
const error = ref("");
function connect() {
  try {
    const target = gatewayInvitation(invitation.value);
    invitation.value = "";
    error.value = "";
    window.location.assign(target);
  } catch (value) { error.value = value instanceof Error ? value.message : "초대 링크를 확인하세요."; }
}
</script>
<template>
  <main>
    <img :src="icon" alt="" width="64" height="64" />
    <h1>Rhyme Terminal</h1>
    <p>PC의 터미널을 여기서 이어서 사용하세요.</p>
    <ol><li>PC 앱의 모바일 연결을 켭니다.</li><li>같은 네트워크 또는 Tailscale에 연결합니다.</li><li>초대 링크를 붙여넣고 PC에서 연결을 승인합니다.</li></ol>
    <form @submit.prevent="connect">
      <label for="invitation">PC 초대 링크</label>
      <textarea id="invitation" v-model="invitation" rows="3" placeholder="http://…/#invite=…" autocomplete="off" autocapitalize="off" spellcheck="false"></textarea>
      <p v-if="error" role="alert" class="error">{{ error }}</p>
      <button type="submit" :disabled="!invitation.trim()">PC에 연결</button>
    </form>
    <small>초대 링크와 인증 정보는 이 화면에 저장하지 않습니다. PC의 터미널은 연결을 끊어도 계속 실행됩니다.</small>
  </main>
</template>
<style>
:root { font-family: system-ui, sans-serif; color: #dce7ef; background: #090d12; color-scheme: dark; }
* { box-sizing: border-box; }
body { margin: 0; }
main { width: min(100%, 520px); margin: auto; padding: max(40px, env(safe-area-inset-top)) 24px 32px; }
h1 { margin: 20px 0 8px; font-size: 28px; }
p, li, small { color: #a5b4c4; line-height: 1.7; }
ol { padding-left: 22px; margin: 28px 0; }
li { margin: 10px 0; }
label { display: block; margin-bottom: 10px; font-weight: 600; }
textarea { width: 100%; padding: 14px; border: 1px solid #344456; border-radius: 10px; background: #111923; color: inherit; font: inherit; resize: vertical; }
textarea:focus { outline: 2px solid #82d3ff; outline-offset: 2px; }
button { width: 100%; margin: 16px 0 24px; border: 0; border-radius: 10px; padding: 15px; font: inherit; font-weight: 600; background: #82d3ff; color: #09131d; }
button:disabled { opacity: .45; }
.error { color: #ffb4ab; }
small { display: block; font-size: 12px; }
</style>
