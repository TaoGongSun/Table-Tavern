# desktop-csp — 桌面版補 CSP（方案）

案別：小案（核心估 100–120 行，預算 100–140），碰信任邊界 → 照「流程上限」做方案送審。

## 關鍵事實
- **srcdoc 繼承宿主 CSP**（沙盒也一樣）。桌面卡片介面目前是 `sandbox="allow-scripts"` + `srcDoc`（ShellFrames.tsx），宿主一上 `script-src 'self'`，卡片殼的行內 `<script>`（含我們的墊片、jQuery／lodash）全被擋。所以「先只上 script-src」這一步單獨上就會打壞卡片介面，iframe 改道必須同一步做。
- **Tauri 對 `tauri://` 送出的每一份 `.html` 都蓋同一條 CSP 標頭**（tauri `manager/mod.rs` `get_asset`→`set_csp`），header 與 meta 取交集；dist 裡放一份 sandbox.html 也拿不到較寬政策。
- Tauri 自動補 nonce／hash：只針對打包 HTML 裡的行內 `<script>`（hash）與 `<style>`、外部 http 腳本（nonce），並自動補 `'self'`。我們的 index.html 沒有行內腳本與 `<style>`，所以實際就是設定值。
- 開發模式（`tauri dev`）頁面直接由 Vite 的 devUrl 提供，不經 Tauri 資產處理，**沒有 CSP**；違規只能用 `harness:build` 的打包版看到。`devCsp` 不設。
- 官方文件（v2.tauri.app/security/csp）建議 IPC 用 `connect-src ipc: http://ipc.localhost`。

## 最終 CSP（`app.security.csp`，用指令表物件寫，方便測試通道設定只覆寫一條）
```
default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline';
img-src 'self' data: https: http:; font-src 'self';
connect-src ipc: http://ipc.localhost https://openrouter.ai;
frame-src tt-sandbox: http://tt-sandbox.localhost; worker-src 'self';
object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
```
| 指令 | 理由 |
|---|---|
| `script-src 'self'` | 主防護：注入的 `<script>`、`onerror=` 等行內處理器、`javascript:` 全擋；打包產出只有 `/assets/*.js`。主 bundle 無 eval／new Function／WebAssembly，不給 `'unsafe-eval'` |
| `style-src 'self'` | CSS 全是打包檔。React `style={}` 走 CSSOM，不受 CSP 管 |
| `style-src-attr 'unsafe-inline'` | 訊息 HTML 的 `style=` 屬性經 `dangerouslySetInnerHTML`（atoms.tsx）進來，已由消毒器限成字色字型類（story-html-style-attr），與網頁版同 |
| `img-src … https: http:` | 桌面版訊息本來就收 http(s) 外部圖（story-markdown.ts `ALLOWED_IMAGE_SRC`，既有產品決定，非暫時放寬）；卡圖、生圖、圖庫都是 Rust 讀檔回 base64 的 `data:`；沒有 blob:、沒開 asset 協定 |
| `connect-src` | 前端只有兩支 fetch（OpenRouter 模型清單、生圖模型清單）；AI 對話、更新、key 檢查都在 Rust。`ipc:`／`http://ipc.localhost` 是 Tauri IPC（macOS／Windows 兩種寫法） |
| `frame-src` | 只准卡片沙盒的自訂協定（macOS 形式／Windows 形式） |
| `worker-src 'self'` | MVU 值解析 Worker 是同源打包檔 |
| 其餘 | 與網頁版 hostCsp 對齊 |

另一處要配合：`react-easy-crop` 執行時會插 `<style>`（會被 `style-src 'self'` 擋），改傳 `disableAutomaticStylesInjection` 並 import 套件自帶的 `react-easy-crop/react-easy-crop.css`。

## 卡片介面 iframe 怎麼不被擋（方案審第 1 輪後改為巢狀）
- 沙盒外層由 Rust 自訂協定 `tt-sandbox` 提供（`Builder::register_uri_scheme_protocol`；Tauri 自己的 isolation 模式也是這樣給 iframe 獨立政策）：回應帶網頁版沙盒同款寬政策 `default-src * data: blob: 'unsafe-inline' 'unsafe-eval'; form-action 'none'`（等同桌面版現況的無 CSP），不經 Tauri 資產處理所以不被蓋主 CSP。Tauri 區分靠「每個回應各帶各的標頭」，不是看來源。
- **卡片碼不跑在外層**，跑在外層裡的內層 `<iframe sandbox="allow-scripts" srcdoc>`：內層繼承外層的寬政策（srcdoc 繼承反過來為我們所用），網址是 `about:srcdoc`。
- 外層 `tt-sandbox` 文件約 25 行，只做固定轉送：收到唯一一次 `load` 後 `inner.srcdoc = html`，之後 `parent` 來的訊息轉給 `inner.contentWindow`、`inner.contentWindow` 來的轉給 `parent`；不 `document.write`、不 `innerHTML`、不 eval 任何東西。內層 100% 撐滿外層。
- 前端：宿主 iframe 改 `src={convertFileSrc("", "tt-sandbox")}`（API 自動給 macOS `tt-sandbox://localhost/`、Windows `http://tt-sandbox.localhost/`），仍掛 `sandbox="allow-scripts"`；首次 load 才送文件。buildShellDocument、ready 握手、key 換殼重掛不變；宿主核的 `event.source` 是外層，正是登記的 contentWindow。
- postMessage 守門（沿用網頁版，全部保留）：外層固定 `host = window.parent`、核 `event.source === host`、`source === "table-tavern-host"`、`kind === "load"`、`token`／`html` 皆字串、收一次即移除該監聽；`window.origin !== "null"`（不是不透明來源）就什麼都不做。宿主只向對應那支 contentWindow 首載送文件。任何一層都不加 `allow-same-origin`；兩層各是不同的不透明來源，DOM 互不可及。
- 待實測確認：外層政策的 `frame-src`（由 `default-src *` 推得）是否放行 `about:srcdoc`（`*` 不匹配 `about:`）；不放行就在外層政策補 `frame-src * about:`。

### IPC 面（Sol 第 1 輪必改的回應）
- Sol 指出 Windows（wry 0.57.0）的初始化腳本會注入子框架，所以「子框架拿不到 invoke key」不能當依據。查 Tauri 2.12.1 原始碼後的兩條路：
  - fetch 路徑：`ipc/protocol.rs` `parse_invoke_request` 以 `Url::parse(Origin)` 解析，沙盒的 `Origin: null` 解析失敗 → 回 500，Tauri 本身已拒。
  - postMessage 路徑：判斷 local 用的是**送訊框架的網址**（Windows 是 `WebMessageReceived` 的 `args.Source`，macOS 是 `frameInfo.request.URL`），而 `is_local_url` 把 Builder 或外掛註冊的所有自訂協定都當 local。`InvokeMessage` 不帶送訊網址，指令處理器與外掛都看不到來源，**「Rust 端明確拒 tt-sandbox／null 來源」在 Tauri 公開 API 做不到**；用 headers 判斷也不行（postMessage 路徑的 headers 由 JS 帶，可偽造）。
  - 所以原方案「卡片碼直接住在 tt-sandbox」在 Windows 反而比現況差（現況卡片碼在 `about:srcdoc` → Remote → 無 remote capability → `!is_local && acl.is_none()` 拒）。巢狀後卡片碼回到 `about:srcdoc`／Remote，與現況同一道保護；Windows 上內層就算拿得到 invoke key 也被 Tauri 的 Remote 拒絕擋下。外層雖是 local，但只跑上面那段固定轉送碼。
- 單元測試（`tauri` 的 `test` 功能，mock runtime，不靠平台 webview，Windows CI 也跑）：以 `get_ipc_response` 送 `InvokeRequest`，`url = about:srcdoc` 的探測指令必被拒；`url = tt-sandbox://localhost/`（Windows 用 `http://tt-sandbox.localhost/`）會被接受——這條是記錄「外層絕不能跑卡片碼」的理由。

## 核心行數估計（不含測試）
Rust 協定模組＋lib.rs 註冊 ~30、外層轉送文件 ~30、ShellFrames 改 src＋首載送文件 ~20、tauri.conf CSP ~15、react-easy-crop 2、測試通道設定 ~3、內層樣式 ~3 → **約 100–120 行**（預算 100–140）。測試：ShellFrames 既有測試改走首載送文件、外層守門單元測試、上面的 IPC mock 測試、測試通道探測指令（harness 功能限定）。

## 測試通道
- 通道的 eval 在頁面裡用 `new AsyncFunction` 建程式（src-tauri/src/harness/eval.rs），沒有 `'unsafe-eval'` 會失效。做法：`tauri.harness.conf.json` 只覆寫 `script-src` 為 `'self' 'unsafe-eval'`（指令表物件 + JSON merge patch）。不影響負面驗證（`'unsafe-eval'` 不放行行內 `<script>` 與事件處理器）；正式版不帶。
- 之前「token 取自 srcdoc」的代測寫法改取 `data-shell-key`（＝token）。
- 新增 harness 功能限定的探測指令（呼叫一次計數加一）與讀計數的通道端點。

## 驗證計畫（harness:build 打包版）
1. 宿主頁先掛 `securitypolicyviolation` 收集器；內層的違規不冒到宿主，由測試卡在內層自己收集、經轉送回報（無 document.write，不會被 document.open 清掉）。
2. 正面：大廳卡圖、一般桌開聊與訊息內 style／外部圖、卡片介面桌（含 MVU 卡，會起值解析 Worker；確認內層 srcdoc 有載出）、圖庫與生圖顯示、卡片編輯器裁切框、OpenRouter 模型清單載入；功能正常且收集器 0 筆。
3. 宿主負面（刻意繞過 DOMPurify，測的是 CSP 不是消毒器）：通道 eval `insertAdjacentHTML` 插 `<img src=x onerror="window.__x=1">`，再插一支 `createElement('script')`＋`textContent` 的行內腳本，確認 `__x`、`__y` 未設、收集器記到 `script-src-attr`／`script-src-elem` 違規。
4. 沙盒負面（macOS）：一張探測測試卡走正常卡片介面路徑，在內層依序試 `__TAURI_INTERNALS__.invoke`、直接 `fetch('ipc://localhost/<探測指令>')`、`webkit.messageHandlers.ipc.postMessage`、`window.ipc?.postMessage`，都打探測指令；宿主經通道讀計數必須是 0（不是只看 API 在不在或逾時）。
5. 正式 CSP 啟動冒煙：不套測試通道的 `script-src` 覆寫另打一包，啟動後 `/shot`（原生截圖，不靠頁面 eval）看到大廳卡片載出，證明 bundle 在 `script-src 'self'` 下能跑、IPC 正常。
6. Windows 的 `http://tt-sandbox.localhost` 形式與沙盒負面探測本機測不到，排實測佇列。

## 施工結果（2026-10-29）
- 核心約 110 行（card_sandbox.rs 非測試 25、外層文件 40、lib.rs 註冊 4、tauri.conf CSP 15、測試通道設定 8、ShellFrames 15、裁切框 3）；測試約 150 行（Rust IPC／協定 65、覆蓋層 14、WebKit 外層轉送 48、通道探測指令 23）。
- 測試通道設定除 `script-src` 加 `'unsafe-eval'`，`style-src` 也加 `'unsafe-inline'`：通道的 motion 外掛會往主頁插 `<style>` 把動畫歸零。正式版兩者都不帶。
- IPC mock 單元測試用 `tauri::test::INVOKE_KEY`（有效 key）：`about:srcdoc` 被 ACL 拒（訊息含 not allowed），同一把 key 的 tt-sandbox 網址通過。
- 測試通道實測（macOS，打包版）：宿主負面 `__x`、`__y` 未設、記到 `script-src-elem`、`script-src-attr` 兩筆；WestFantsy 卡片介面照常畫出（內層 srcdoc、可捲動）；訊息行內 style 生效（紅字粗體）、外部 https 圖載入；大廳卡圖與封面（data:）、裁切框、OpenRouter 生圖模型清單（200、63 筆）、MVU 值解析 Worker 啟動與求值皆正常，收集器 0 筆。
- 沙盒負面（macOS）：宿主先打一次探測指令（計數 1）並從 IPC 標頭攔下真 invoke key 傳給探測卡；內層（origin null、網址 about:srcdoc）沒有 `__TAURI_INTERNALS__`／`window.ipc`，`fetch(ipc://)` 失敗，`webkit.messageHandlers.ipc.postMessage` 帶真 key 送出後計數仍是 1。探測卡由通道在宿主建同位置的 iframe（宿主 sandbox iframe → tt-sandbox 外層 → 內層 srcdoc）送入，未經卡片匯入流程。
- 正式 CSP 冒煙：不帶通道覆寫另打一包，啟動後原生截圖大廳與範例桌正常；通道 eval 被 CSP 擋（`unsafe-eval` 未放行），標頭顯示 `script-src 'self'` 加 Tauri 自帶的 hash。
- 生圖顯示未用真模型生圖驗：生圖結果與卡圖同走 Rust 讀檔回 `data:`，以卡圖與封面代驗。

## 觀察
- 測試通道回歸腳本 `scripts/harness-e2e.mjs` 已過時：第 3 步等的「GM 推進」按鈕在新版開新桌畫面上找不到（介面改版所致，與本案無關）。
