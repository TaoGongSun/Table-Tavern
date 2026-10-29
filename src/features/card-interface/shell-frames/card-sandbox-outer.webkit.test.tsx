// 真實 WebKit 裡的沙盒外層（src-tauri/src/card_sandbox.html；npm run test:webkit）：收一次 load 放進內層 srcdoc，
// 之後只雙向轉送；再送 load 不會換掉內層。這裡用 srcdoc 掛外層（同樣是不透明來源），測的是轉送碼本身。
import { afterEach, describe, expect, it } from "vitest";
import outer from "../../../../src-tauri/src/card_sandbox.html?raw";

const card = (name: string) => `<!DOCTYPE html><html><body><script>
  parent.postMessage({ source: "table-tavern-card", kind: "boot", name: "${name}" }, "*");
  addEventListener("message", function (event) {
    if (event.source !== parent) return;
    parent.postMessage({ source: "table-tavern-card", kind: "echo", name: "${name}", got: event.data.kind }, "*");
  });
</script></body></html>`;

let frame: HTMLIFrameElement | null = null;
afterEach(() => frame?.remove());

describe("沙盒外層轉送（真實 WebKit）", () => {
  it("首次 load 放進內層；內層訊息轉到宿主、來源是外層；宿主訊息轉進內層；第二次 load 不換內層", async () => {
    const seen: Array<{ data: Record<string, unknown>; fromOuter: boolean }> = [];
    const onMessage = (event: MessageEvent) => {
      if (event.data?.source === "table-tavern-card") seen.push({ data: event.data, fromOuter: event.source === frame?.contentWindow });
    };
    window.addEventListener("message", onMessage);
    const waitFor = async (check: () => boolean) => {
      for (let i = 0; i < 100 && !check(); i += 1) await new Promise((resolve) => setTimeout(resolve, 20));
      expect(check()).toBe(true);
    };
    frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = outer;
    await new Promise((resolve) => {
      frame!.addEventListener("load", resolve, { once: true });
      document.body.appendChild(frame!);
    });
    const view = frame.contentWindow!;
    // 型別不對的 load 不收
    view.postMessage({ source: "table-tavern-host", kind: "load", token: 1, html: card("bad") }, "*");
    view.postMessage({ source: "table-tavern-host", kind: "load", token: "t", html: card("first") }, "*");
    await waitFor(() => seen.some(({ data }) => data.kind === "boot"));
    view.postMessage({ source: "table-tavern-host", kind: "load", token: "t", html: card("second") }, "*");
    view.postMessage({ source: "table-tavern-host", kind: "chat", token: "t" }, "*");
    await waitFor(() => seen.some(({ data }) => data.kind === "echo"));
    window.removeEventListener("message", onMessage);
    expect(seen.map(({ data }) => `${data.kind}:${data.name}`)).toEqual(["boot:first", "echo:first"]);
    expect(seen.map(({ data }) => data.got ?? null)).toEqual([null, "chat"]);
    expect(seen.every(({ fromOuter }) => fromOuter)).toBe(true);
  });
});
