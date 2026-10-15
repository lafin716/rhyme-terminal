import { describe, expect, it } from "vitest";
import { gatewayInvitation } from "./invitation";
describe("native gateway invitation", () => {
  it("preserves the gateway origin and token without query parameters", () => {
    const url = new URL(gatewayInvitation("http://100.70.1.2:43123/#invite=abc"));
    expect(url.origin).toBe("http://100.70.1.2:43123");
    expect(url.search).toBe("");
    expect(new URLSearchParams(url.hash.slice(1)).get("invite")).toBe("abc");
    expect(new URLSearchParams(url.hash.slice(1)).get("native")).toBe("android");
  });
  it.each(["javascript:alert(1)", "file:///etc/passwd", "http://user:pass@host/#invite=x", "http://host/path#invite=x", "http://host/?invite=x", "http://host/"])("rejects %s", raw => {
    expect(() => gatewayInvitation(raw)).toThrow();
  });
});
