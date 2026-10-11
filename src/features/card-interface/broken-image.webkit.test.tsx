// 真實 WebKit 沙盒裡的破圖收斂（npm run test:webkit，不進 verify）：壞圖藏起來、卡片 onerror 換成有效網址後恢復。
// 沙盒沒有 same-origin、碰不到 iframe 內 DOM：殼自己量好結果，經 storage 墊片（localStorage 寫入會回報宿主）送出來。
import { afterEach, describe, expect, it } from "vitest";
import { buildShellDocument } from "./interface-card";

const PIXEL =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
const BROKEN = "data:image/png;base64,AAAA";

const shell = `<!DOCTYPE html><html><head></head><body>
<img id="dead" src="${BROKEN}" width="40" height="30" alt="破圖">
<img id="fixed" src="${BROKEN}" width="40" height="30" onerror="if (!this.dataset.retried) { this.dataset.retried = '1'; this.src = '${PIXEL}'; }">
<img id="fine" src="${PIXEL}" width="40" height="30">
<script>
  window.addEventListener("load", function () {
    setTimeout(function () {
      var result = {};
      ["dead", "fixed", "fine"].forEach(function (id) {
        var img = document.getElementById(id);
        var rect = img.getBoundingClientRect();
        result[id] = {
          broken: img.hasAttribute("data-tt-broken"),
          visibility: getComputedStyle(img).visibility,
          size: [rect.width, rect.height],
        };
      });
      localStorage.setItem("result", JSON.stringify(result));
    }, 300);
  });
</script>
</body></html>`;

let frame: HTMLIFrameElement | null = null;
afterEach(() => frame?.remove());

describe("破圖收斂（真實 WebKit 沙盒）", () => {
  it("壞圖藏起來、尺寸照留（版面不塌）；卡片換成有效網址後恢復；正常圖不動", async () => {
    const result = new Promise<Record<string, { broken: boolean; visibility: string; size: number[] }>>((resolve) => {
      window.addEventListener("message", (event) => {
        const data = event.data;
        if (data?.source === "table-tavern-card" && data.kind === "storage" && data.entries?.result) {
          resolve(JSON.parse(data.entries.result));
        }
      });
    });
    frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = buildShellDocument(shell);
    document.body.appendChild(frame);
    expect(await result).toEqual({
      dead: { broken: true, visibility: "hidden", size: [40, 30] },
      fixed: { broken: false, visibility: "visible", size: [40, 30] },
      fine: { broken: false, visibility: "visible", size: [40, 30] },
    });
  });
});
