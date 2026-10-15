/** Navigate to the gateway itself so its exact Host/Origin policy is unchanged. */
export function gatewayInvitation(raw: string): string {
  const url = new URL(raw.trim());
  if (url.protocol !== "http:" && url.protocol !== "https:") throw new Error("HTTP 또는 HTTPS 초대 링크를 입력하세요.");
  if (url.username || url.password || url.pathname !== "/" || url.search) throw new Error("PC에서 복사한 초대 링크를 입력하세요.");
  const invite = new URLSearchParams(url.hash.slice(1)).get("invite");
  if (!invite || invite.length > 512) throw new Error("초대 코드가 없습니다. PC에서 새 초대 링크를 복사하세요.");
  // Marker is only for the return control; it grants no server or native rights.
  url.hash = new URLSearchParams({ invite, native: "android" }).toString();
  return url.toString();
}
