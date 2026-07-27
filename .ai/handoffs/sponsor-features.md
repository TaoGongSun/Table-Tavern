# Handoff: sponsor-features

## Current state
三項全部實作完成（作者頁、配色 +5、AI 生成角色圖），cargo 85 綠＋npm build 綠，等使用者實測。待討論議程：Ko-fi 導購歧義（見任務檔）。

## Completed
- AI 生圖後端：`generate_character_image(world, name, extraPrompt, source)` 回圖片 data URL。api 來源走 OpenRouter 專用 Images API（POST {base}/images，aspect_ratio 2:3、resolution 1K，模型讀 preferences.image_model、預設 google/gemini-3.1-flash-image，回應取 data[0].b64_json）；CLI 來源照送請求（prompt 加「能生圖就存 PNG 回絕對路徑」指示，codex 加 `$imagegen` 前綴），回覆掃 data URL 或存在的圖片路徑（extract_image_from_text），掃不到回錯。追加描寫存卡（gen_prompt frontmatter 欄位，換行轉空白）。stream_via_transport 加 transport_override 參數供生圖指定與聊天不同的連線。
- AI 生圖前端：卡片編輯器「✨ AI 生成」鈕 → 生成對話框（追加描寫預填上次值＋生圖來源下拉：OpenRouter API＋偵測到的 CLI，記住上次選擇 image_source）→ 成功餵 setPendingImage 接既有 2:3 裁切→存檔流；失敗顯示道歉訊息＋後端錯誤小字、不扣次數。免費 3 次（preferences.ai_image_trials_used，成功才 +1），用完未贊助 → 介紹 modal（文案＋Ko-fi 鈕）。設定 AI 分頁加「生圖模型」欄位。
- 四家 CLI 生圖實測（2026-07-27）：codex ✓（`$imagegen`，需信任目錄，實測出 1254×1254 PNG）；agy ✓（原生，存自家 scratch 回絕對路徑）；grok ✓ 能力在（原生 image_gen，實測時 403 額度）；claude ✗。參考文件：.ai/reference/CODEX_IMAGE_GENERATION_GUIDE.md。
- 配色 +5：App.css 五套 token 區塊（parchment 羊皮紙＝Solarized Light 色相加深墨色／herbal 藥草坊／candlelight 燭光／port 波特酒／seamist 海霧）；App.tsx resolveTheme（未知值或未解鎖 sponsor 主題一律回 dark）＋色票選擇器（☕ 角標、aria-pressed）＋試看機制（previewTheme state，effect cleanup 關窗即復原）＋試看提示行附 Ko-fi 鈕；i18n zh/en 六鍵。解鎖旗標暫讀 `preferences.sponsor_unlocked === true`（測試可手改 config.json），正式憑證匯入等贊助包格式定案。
- 設定視窗新增第三分頁「作者」：頭像（Tao-icon.png，圓形 6rem）＋ 作者名 ＋ 一句文案 ＋「☕ 請作者喝咖啡」鈕（openUrl 開系統瀏覽器）
- 分頁切換沿用 AI 分頁未儲存確認（switchTab 統一處理 appearance／author）
- i18n zh-TW／en 三鍵：authorTab、authorBlurb、sponsorBtn
- 頭像資產：Tao-icon.png（240×240）複製為 src/assets/tao-icon.png，Vite 打包

- 生成歷史圖庫（2026-07-27 實測後追加）：生成原圖落地 `{world}/gen-gallery/{name}/{unix_ms}.png`（不設上限）；生成成功不再自動開裁切，改刷新對話框內縮圖列（新在前、每批 12 張「載入更多」）；點縮圖＝重開 2:3 裁切重選範圍並套用（同一張可反覆重裁）；縮圖 X＋確認＝真刪。後端三 command（list／read／delete_gallery_image）含路徑逃逸防護與自寫 base64 decoder。生圖 UX 另修：生成鈕右下角主色（置頂例外拍板）、✨ 鈕描邊、來源旁「⚙ AI 連線」直開設定 ai 分頁（設定視窗移版面尾端修疊層）、CLI 生圖呼叫開工具權限（codex workspace-write／agy skip-permissions／grok always-approve，聊天路徑不動）。

## Verification
- 後端：`cargo test` 85 綠（基線 77，+8 新測試：Images API mock 兩案、extract_image_from_text 三案、gen_prompt roundtrip 等）；`cargo clippy --all-targets` 0 error
- 前端：`npm run build` exit 0
- CLI 生圖能力：真發請求實測（見 Completed 第三條），非自我宣稱
- 生成歷史圖庫：2026-07-28 使用者實機通過（Gemini CLI 來源生圖 → 縮圖列出現 → 套用到卡片，側欄顯示新圖）。在 stable-id-storage 改成代碼定址後複驗，圖庫路徑已改到世界目錄內。張數受免費 3 次限制，未大量驗證載入更多。

## Remaining
- 使用者實測三項（尤其 AI 生圖各來源實跑）
- 「匯入贊助包」入口未做（等贊助包檔案格式定案，預計放作者頁）
- 未解鎖介紹 modal 目前純文字＋Ko-fi 鈕，範例圖等功能上線後生幾張好圖再補
- Ko-fi 導購歧義方案待討論（任務檔議程）

## Next action
1. 拍板任務檔「待討論議程」兩項：常用提示詞標籤（主線建議 v1 內建精選清單，分析見任務檔）、Ko-fi 導購歧義。
2. 測試贊助旗標：手改 config.json `preferences.sponsor_unlocked=true`；重置免費次數改 `ai_image_trials_used`。

（2026-07-27 晚：本對話已收工交接，新對話從此檔接手即可，無未存現場。）
