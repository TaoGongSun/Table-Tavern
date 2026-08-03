# Handoff: sponsor-features

## Current state
三項全部實作完成（作者頁、配色 +5、AI 生成角色圖），cargo 85 綠＋npm build 綠，等使用者實測。**待討論議程已全數結案（2026-08-03）**，本任務唯一剩餘＝實機驗收。

## Completed
- AI 生圖後端：`generate_character_image(world, name, extraPrompt, source)` 回圖片 data URL。api 來源走 OpenRouter 專用 Images API（POST {base}/images，aspect_ratio 2:3、resolution 1K，模型讀 preferences.image_model、預設 google/gemini-3.1-flash-image，回應取 data[0].b64_json）；CLI 來源照送請求（prompt 加「能生圖就存 PNG 回絕對路徑」指示，codex 加 `$imagegen` 前綴），回覆掃 data URL 或存在的圖片路徑（extract_image_from_text），掃不到回錯。追加描寫存卡（gen_prompt frontmatter 欄位，換行轉空白）。stream_via_transport 加 transport_override 參數供生圖指定與聊天不同的連線。
- AI 生圖前端：卡片編輯器「✨ AI 生成」鈕 → 生成對話框（追加描寫預填上次值＋生圖來源下拉：OpenRouter API＋偵測到的 CLI，記住上次選擇 image_source）→ 成功餵 setPendingImage 接既有 2:3 裁切→存檔流；失敗顯示道歉訊息＋後端錯誤小字、不扣次數。免費 3 次（preferences.ai_image_trials_used，成功才 +1），用完未贊助 → 介紹 modal（文案＋Ko-fi 鈕）。設定 AI 分頁加「生圖模型」欄位。
- 四家 CLI 生圖實測（2026-07-27）：codex ✓（`$imagegen`，需信任目錄，實測出 1254×1254 PNG）；agy ✓（原生，存自家 scratch 回絕對路徑）；grok ✓ 能力在（原生 image_gen，實測時 403 額度）；claude ✗。參考文件：.ai/reference/CODEX_IMAGE_GENERATION_GUIDE.md。
- 配色 +5：App.css 五套 token 區塊（parchment 羊皮紙＝Solarized Light 色相加深墨色／herbal 藥草坊／candlelight 燭光／port 波特酒／seamist 海霧）；App.tsx resolveTheme（未知值或未解鎖 sponsor 主題一律回 dark）＋色票選擇器（☕ 角標、aria-pressed）＋試看機制（previewTheme state，effect cleanup 關窗即復原）＋試看提示行附 Ko-fi 鈕；i18n zh/en 六鍵。解鎖狀態 2026-07-28 起改由贊助包檔案（`.ttpack`）推導，見 [release-4-theme-pack](release-4-theme-pack.md)。
- 設定視窗新增第三分頁「作者」：頭像（Tao-icon.png，圓形 6rem）＋ 作者名 ＋ 一句文案 ＋「☕ 請作者喝咖啡」鈕（openUrl 開系統瀏覽器）
- 分頁切換沿用 AI 分頁未儲存確認（switchTab 統一處理 appearance／author）
- i18n zh-TW／en 三鍵：authorTab、authorBlurb、sponsorBtn
- 頭像資產：Tao-icon.png（240×240）複製為 src/assets/tao-icon.png，Vite 打包

- 生成歷史圖庫（2026-07-27 實測後追加）：生成原圖落地 `{world}/gen-gallery/{name}/{unix_ms}.png`（不設上限）；生成成功不再自動開裁切，改刷新對話框內縮圖列（新在前、每批 12 張「載入更多」）；點縮圖＝重開 2:3 裁切重選範圍並套用（同一張可反覆重裁）；縮圖 X＋確認＝真刪。後端三 command（list／read／delete_gallery_image）含路徑逃逸防護與自寫 base64 decoder。生圖 UX 另修：生成鈕右下角主色（置頂例外拍板）、✨ 鈕描邊、來源旁「⚙ AI 連線」直開設定 ai 分頁（設定視窗移版面尾端修疊層）、CLI 生圖呼叫開工具權限（codex workspace-write／agy skip-permissions／grok always-approve，聊天路徑不動）。

- 構圖二選一（2026-07-30，取代「常用提示詞標籤」議程）：生圖對話框加「構圖」全身／半身（預設全身，記 `preferences.image_framing`），後端 `generate_character_image` 收 `framing`，prompt 開頭 `full-body`／`waist-up half-body`；追加描寫改標「takes priority over the defaults above」，讓玩家用文字推翻素背景等預設。2:3 直式不放開（角色卡版面吃這個比例）。舊前端沒傳參數＝全身。

- CLI 生圖路徑解析修正＋中轉檔清理（2026-08-01）：回覆改先「整行掃」——從路徑起點（POSIX `/` 或 Windows `C:\`）吃到最後一個圖片副檔名，含空格的路徑與尾隨標點都不再被切斷，逐詞切法留作保底、候選去重（`extract_image_refs`／新增 `path_span`）；相對路徑改以 CLI 工作目錄為基準解析（絕對路徑 join 後不變）。生圖收尾一律清掉工作目錄裡的圖片中轉檔（成功與失敗都清，三家 CLI 通用，遞迴進 CLI 自開的子目錄、清空即移除，Windows 檔案被佔用時跳過），工作目錄準備抽成 `cli_workspace` 供聊天與生圖共用。

- 生圖失敗訊息分流（2026-08-01）：CLI prompt 改問兩個暗號——`NO_IMAGE`（根本不會生圖）與 `REFUSED`（不肯生這一張），前端 `explainAiError` 各對一句人話（新增十語系 `errRefused`）；模型不照暗號回時，再用拒絕字樣（content policy／can't generate／無法生成／拒絕等）保底歸類。兩者都沒對上時，錯誤小字附上 CLI 最後一句原話（截 200 字，`last_sentence`），不再只顯示「回覆中沒有圖片」。

## Verification
- 生圖失敗訊息分流（2026-08-01）：codex 實測比對——同一段被拒的描述，只給 `NO_IMAGE` 選項時它回 `NO_IMAGE`（會被誤讀成「來源不會生圖」），加上 `REFUSED` 選項後改回 `REFUSED`，分流可靠。`cargo test` 126 綠、`npx tsc --noEmit` 0 錯、`npm run check:i18n` 十語系 OK、`npm run build` exit 0；08-01 15:0x 使用者實機驗收通過。
- CLI 生圖路徑修正（2026-08-01）：`cargo test` 126 綠（+4：macOS 含空格路徑、Windows 反斜線含空格路徑、CLI 相對路徑、中轉檔清理遞迴且只刪圖片）；clippy／fmt 與改動前逐項相同（既有 6 項與本次無關）。08-01 14:54 使用者實機驗收通過：codex 出圖進圖庫（md5 與 `~/.codex/generated_images/` 該次一致），cli-workspace 清空無殘留。codex 回報路徑有三種形態，都要接：①`~/.codex/generated_images/` 原始絕對路徑（無空格，舊版剛好會過）②複製到 `cli-workspace/` 的絕對路徑（被 `Application Support` 的空格切碎）③複製到 `cli-workspace/output/imagegen/` 後回相對路徑（要補工作目錄基準）。它照 `~/.codex/skills/.system/imagegen/SKILL.md` 的規定不把成品留在自己家，所以②③會出現。
- 構圖二選一（2026-07-30）：`npx tsc --noEmit` 0 錯、`npm run check:i18n` 十語系 OK、`cargo test --lib` 117 綠、`npm run build` exit 0。實際出圖待使用者實機。
- 後端：`cargo test` 85 綠（基線 77，+8 新測試：Images API mock 兩案、extract_image_from_text 三案、gen_prompt roundtrip 等）；`cargo clippy --all-targets` 0 error
- 前端：`npm run build` exit 0
- CLI 生圖能力：真發請求實測（見 Completed 第三條），非自我宣稱
- 生成歷史圖庫：2026-07-28 使用者實機通過（Gemini CLI 來源生圖 → 縮圖列出現 → 套用到卡片，側欄顯示新圖）。在 stable-id-storage 改成代碼定址後複驗，圖庫路徑已改到世界目錄內。張數受免費 3 次限制，未大量驗證載入更多。

## Remaining
- 使用者實測三項（尤其 AI 生圖各來源實跑）
- 未解鎖介紹 modal 目前純文字＋Ko-fi 鈕，範例圖等功能上線後生幾張好圖再補（不擋結案）
- 議程已清空：提示詞標籤（07-30 構圖二選一）、色情詞句失敗處理（08-01 失敗訊息分流）、Ko-fi 導購歧義（08-03 連結已直指商品頁）三項皆結案

## Next action
1. 使用者實機驗收構圖二選一：生圖對話框選「半身」→ 確認出圖是腰以上特寫、2:3 直式不變、重開對話框記住上次選擇。
3. 測試贊助狀態：把 `.ttpack` 丟進「文件/TableTavern」（或作者頁匯入），刪檔即還原；重置免費次數改 `ai_image_trials_used`（手改 config.json 的舊旗標已失效）。

（2026-07-27 晚：本對話已收工交接，新對話從此檔接手即可，無未存現場。）
