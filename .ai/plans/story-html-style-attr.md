# story-html-style-attr — 方案

案別：小案（核心估 ≤80 行，不含測試、i18n）；碰信任邊界輸入，方案送審。測試 ≤160 行。

## 做法
- `src/shared/ui/story-markdown.ts`（驗證本體在 `story-inline-style.ts`）：`style` 加進 `STORY_MARKDOWN_ALLOWED_ATTR`；在既有專用實例 `getStoryPurifier()` 上加一個 `uponSanitizeAttribute` 掛鉤，`attrName === "style"` 時把 `data.attrValue` 換成 `sanitizeInlineStyle()` 的結果，結果為空就 `data.keepAttr = false`（不留 `style=""`）。
- `sanitizeInlineStyle` 只看原始字串，不解析 CSS、不加依賴、不走 `element.style`（瀏覽器會把顏色正規化成 `rgb()`，jsdom 與 WebKit 的 CSSOM 也不一致）。輸出只由驗過的 token 重組成 `prop: value; …`。
- 桌面版 CSP 是 null，行內 style 直接生效。

## 白名單與值的規則
1. **整條丟棄**：原始值任一處出現 `\`（CSS 跳脫能拼出 `\75rl(`）、引號、`/`、`<`、`>`、`&`、`@`、`{`、`}`，或出現四個顏色函式名以外的 `(`，整個 style 屬性拿掉。
2. 其餘照 `;` 拆段、每段第一個 `:` 拆屬性與值；屬性小寫比對白名單，不在白名單的只丟該段（`position:fixed` 這類因此自然消失）。值尾的 `!important` 去掉後照常驗（ST 卡常寫）。
3. 各屬性的值：
   - `color`、`background-color`：hex（3／4／6／8 位）、純英文字母關鍵字（具名色、`transparent`、`currentcolor`、`inherit`；不認得的字瀏覽器自己當無效）、`rgb()／rgba()／hsl()／hsla()` 括號內只准數字、`.`、`%`、`,`、空白、`deg`（這四個函式不會發請求；卡片底色極常寫 `rgba(0,0,0,.5)`）。（Sol 方案審同意收）
   - `font-weight`：`normal|bold|bolder|lighter|100…900`。
   - `font-style`：`normal|italic|oblique`。
   - `text-decoration`：1–4 個 token，每個是 `none|underline|overline|line-through|solid|double|dotted|dashed|wavy` 或上述顏色值。
   - `font-size`：`px／em／rem／%` 數值與 `larger`（→1.2em）、`smaller`（→0.833em）一律輸出程式產生的 `clamp(8px, 值, 32px)`，限制的是最後計算字級，巢狀倍增也出不了範圍（Sol 方案審必改）；絕對關鍵字 `xx-small…xx-large` 原樣。其餘單位（vw、vh 等）丟該段。
4. 不收：定位與尺寸類（position、top、margin、width…）、`background` 簡寫、`background-image`、`content`、`display` 等一律不在白名單。〔模型判斷·未裁決〕
5. 為什麼蓋不住介面、外連不出去：白名單沒有任何會脫離文件流或改尺寸的屬性；值裡沒有 url／image-set／expression 等函式，也沒有跳脫可拼出它們。

## 網頁版
- 文字段（聊天、匯入預覽、卡片介面的 `MessageBody` 文字段）都在宿主頁以 `dangerouslySetInnerHTML` 插入，吃 `web/site-headers.ts` 的宿主 CSP。
- `hostCsp` 加一條 `style-src-attr 'unsafe-inline'`：只放寬 style 屬性，`style-src 'self'` 不動，`<style>` 元素照擋。理由：消毒器已保證屬性值沒有函式與跳脫，行內屬性沒有外連能力；`img-src`／`font-src` 照舊是第二層。不支援 `style-src-attr` 的舊瀏覽器退回 `style-src 'self'`，只是樣式不生效。
- 「消毒後轉成受控樣式」不採用：顏色是連續值轉不成 class，改用 CSSOM 套用得在三個插入點各加 ref 與 effect，程式更多。
- 將來 desktop-csp 立案時要帶同一條 `style-src-attr`。

## 核心行數估計
`sanitizeInlineStyle` 約 40、掛鉤約 8、白名單與常數約 10、CSP 1 行＋註解 2，合計約 60（上限 80）。

## 測試計畫（jsdom 為主，WebKit 一條）
- 既有 `story-markdown.test.ts:30` 斷言 style 不在白名單改成在；`:104` 的 `position:fixed` 確認整個屬性移除。
- 正常路徑：六屬性各一正常值、hex 3／6／8 位、`rgba(0,0,0,.5)`、大小寫與多餘空白、`!important`、`font-size` 三種單位超界被夾、未知屬性只丟該段、`<font color>` 舊路照舊。
- 繞過嘗試：`background:url(x)`、`color:red;background-image:url(x)`、`color:\75rl(x)`、`color:expression(x)`、`color:red/**/`、`color:image-set(x)`、`rgb(url(x))`、引號值、`position:fixed;top:0`（只剩白名單段或整個屬性消失）。
- WebKit：一條確認消毒後的 `color`／`background-color` 真的套上（computed style）。
- `web/site-headers.test.ts`：宿主 CSP 含 `style-src-attr 'unsafe-inline'`，`style-src 'self'` 照舊。

## 實作結果
核心實際 76 行（`story-inline-style.ts` 66、掛鉤 8、CSP 2，含註解）；測試 86 行（含 WebKit 巢狀字級上下界與顏色 computed style、實體解碼 `&#x75;rl(` 樣本）。verify 13 步全綠、test:webkit 26 過。

## 已知限制
- 排版類行內 style（定位、尺寸、邊距、`background` 簡寫）不生效。〔模型判斷·未裁決〕
- `text-decoration` 不收顏色函式，只收 hex 與具名色。〔模型判斷·未裁決〕
- 網頁版宿主 CSP 下未實測（style 屬性生效、行內 `<style>` 被擋），只有標頭字串測試。〔模型判斷·未裁決〕

## 觀察
（施工中範圍外發現寫這裡）
