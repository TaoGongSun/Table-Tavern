# 目錄結構規範

新增檔案、拆檔、建資料夾時遵守這份。`npm run verify` 的第一步 `check:structure` 會擋掉機器判得準的違規；判不準的（某支檔案該屬哪個功能）靠這份文件與 review。

## 新增功能放哪裡

依序問四個問題：

1. **誰是 owner？** 已經有對應的 `features/<name>/` 或 Rust domain 就放回去，不要在別處另起爐灶。
2. **沒有 owner，但只有一支小檔？** 可以先單檔放在最接近的 feature 裡，不必為它預先開資料夾。
3. **已經形成 UI ＋ 邏輯，或約三支以上同生命週期的檔案？** 升格成 `features/<name>/`。
4. **想放 `shared/`？** 見下方條件，多半還不到時候。

`src/` 根層只放應用程式入口：`App.tsx`、`App.css`、`main.tsx`、`vite-env.d.ts`。新增第五個原始碼檔＝頂層架構變更，必須連 `scripts/check-structure.mjs` 一起改，讓這件事在 review 裡藏不住。

## 什麼時候升格資料夾

前端看訊號，不死看數量：同一功能已有約三支 production 檔、或同時存在 UI ＋ hook ＋ 純邏輯、或為了改一件事得反覆在同一群檔案間跳。兩支檔案但已經是清楚且會繼續長的獨立功能，也可以升格；三支很小又彼此無關的檔案，不必硬塞在一起。

**升格要一次搬齊**：某功能的 view、controller、logic、test，只要 owner 是單一的，就同批進 `features/<name>/`。不接受「邏輯搬了、畫面留在 `views/`」的半套升格——那會讓一個功能散在三個地方，比原本平鋪更難找。`views/` 與 `controllers/` 只放把多個 feature 組起來的外殼，清單寫在 `scripts/check-structure.mjs`，其餘檔案 checker 會擋。

feature 內部不必再機械式拆 `components/`、`hooks/`、`models/`；等這個 feature 自己長到掃不完再說。

Rust 側相反：新 module 先用單檔 `foo.rs`，等它真的長出兩個以上能各自命名的責任，再升格成 `foo/`。不預先建立只有 `mod.rs` 的空殼。`commands/` 只做 Tauri 邊界（接參數、呼叫 domain、整理回傳），核心規則不寫在那裡。

## `shared/` 何時成立

不是預設落點。條件是**至少兩個彼此獨立的 feature 真的在用**，而且它語意上不屬於其中任何一方。「以後可能會共用」不算——先讓第一個 owner 持有，等第二個真實使用者出現再抽。

現有分區：`contracts/`（與 Rust 的型別契約）、`ui/`（跨 feature 的呈現與互動）。不為單支檔案預先開第三個分區。

## 修改既有功能要不要順便搬

發現同一功能的檔案散在錯誤層級時：

- 這次修改的範圍容得下，就一併做小型的 structure-only 搬遷。
- 容不下，就記進 `.ai/BACKLOG.md`，不能假裝沒看到。
- 不為了少改幾行 import，把新的同伴檔繼續丟在錯誤層級。
- 不在功能案裡順手做大規模無關搬家；那要獨立立案。

## 網頁版 `web/`

網頁版是獨立的 Vite 專案：自己一份 `package.json`、lockfile、`tsconfig.json`、`vite.config.ts`、`vitest.config.ts`（建置外掛也放 `web/` 根，例：`mvu-engine-plugin.ts`、產生部署標頭 `_headers` 的 `site-headers.ts`），建置產物 `web/dist/` 是純靜態檔；`web/public/` 放原樣複製的靜態檔（卡片介面的 `sandbox.html`、`404.html`）。

- `web/src/` 套用與 `src/` 同一套規則（features 落點、`shared/` 條件、禁止事項、命名），根層入口同樣只放 `App.tsx`、`App.css`、`main.tsx`、`vite-env.d.ts`；`check-structure` 兩棵樹都檢查。
- 桌面版的純 TS 模組用 `@desktop/*` 路徑別名直接 import，不複製。只引不碰 Tauri（`@tauri-apps/*`、`invoke`）的模組；要引的模組若開始依賴 Tauri，先拆出純邏輯再引。
- 端對端測試放 `web/e2e/`（不在 `web/src/`、不進 verify），含照 Cloudflare Pages 行為託管產物的 `static-server.mjs` 與正式建置的 `smoke-dist.mjs`。
- 根目錄的 vitest 排除 `web/**`，網頁版測試由 verify 的 web 步驟跑。

## 禁止事項

- `utils/`、`helpers/`、`misc/`、`common/`——任意深度都禁。不知道一支檔案放哪，代表它的責任還沒想清楚。
- `temp.ts`、`new.ts`、`foo2.ts` 這類過渡命名進 main。
- 跨目錄的鏡像測試樹。測試跟著它測的東西住。
- 為了讓前端與 Rust 長成同一個形狀而搬檔。兩側各自依責任演化。

## 命名

- React 元件 `PascalCase.tsx`；hook 與 controller `useSomething.ts`（hook 是純邏輯，副檔名用 `.ts`）。
- 純資料、routing、model、formatting 模組用描述性的 `kebab-case.ts`。
- 測試同 stem ＋ `.test.ts` / `.test.tsx`。
- Rust `snake_case.rs`，目錄名與 module 名一致。

大小寫慣例由 review 把關，checker 不擋——既有的 `shared/ui/atoms.tsx` 這類「一檔多個小元件」的合理例外，機器分不出來。

## 檔案大小

單檔盡量不超過約 1000 行、單一資料夾超過約 20 個檔就考慮分類。兩者都是**必須檢查**，不是必須拆：拆依責任，不依行號。200 行混三種責任，比 1100 行的單一資料表更值得拆。不為了湊數字製造 `part1` / `part2`。

## checker 的清單只列固定位置

`scripts/check-structure.mjs` 的清單只列架構上的固定位置（根層入口、組合外殼），禁止逐檔豁免違規。一旦清單變成例外收容所，CI 紅燈的預設解法就會變成「把新檔加進清單」，關卡會退化成橡皮圖章。要改清單就得改這個檔，讓架構變更在 review 裡藏不住。
