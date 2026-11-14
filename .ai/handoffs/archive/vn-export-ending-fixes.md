# vn-export-ending-fixes

VN 實測（B4／B5）挖到的匯出與完結畫面問題。估小到中案。

## 狀態
- 分支 `vn-export-ending-fixes`，未合 main、未壓縮。
- 1 已做：遊戲選單「匯出成 VN 作品」按下先查 `vn_endings`，沒有任何結局（含保底）就直接顯示「還沒有任何一條路線走到結局…」，不開對話框；後端原有檢查留著當最後防線。
- 2 已做：標題畫面一律放主題曲（`VnPlayer` 標題開著時給 `useVnAudio` 的 `bgm` 傳 `null`），開始／繼續後才換成當下那格的配樂；主題曲沒落地就維持現在這首。`plans/vn-audio.md` §2、§3 原地改寫〔模型判斷·未裁決〕。
- 4 已做：結局是保底結局時，完結畫面不出「已達成結局 x / y」（保底不計入是〔作者裁決 2026-11-02〕，沒動）。觀察：進度頁、結局一覽在只達成保底結局時一樣顯示 0 / y，不改。
- 5 不改：匯出成功會用 Finder 指到檔案（`revealItemInDir`），跟 VN 卡、角色卡匯出同一套。
- 6 已做：精靈 ① 的「看大綱」「直接開始」（看過大綱後加「下一步」為主鈕）搬到對話窗底部固定動作列，跟 ②③ 一致；額度提醒放動作列左側；角色卡分頁不出。
- 7 已做：`vn-wizard.ts` 英、西、葡、德、法、俄的生圖張數與結局數改用 plural（`vnArtEstimate`／`vnArtPaused`／`vnArtPausedUnknown`／`vnEndingCount`／`vnArtQuota`／`vnArtFailed`；俄文只有前三個是「{n} 名詞」形式）。
- 8 已做：「換立繪來源」把焦點移到來源選單時一定畫焦點框（沿用 `MoreMenu` 的 `focusFrom`，改成 export）；`art-style.css` 讓這個框只看 `data-focus-ring`，視窗不在前景時也畫。
- 9 不改：測試通道假象。通道點擊前對目標 `scrollIntoView({inline:"center"})`，舞台裡的霧層寬 100%＋960px 讓舞台可被橫向捲動；點右上角背景重試角標時整個舞台被往左捲約 326px，背景右緣露出底色成直向分界、角標跑到中間。玩家用滑鼠點不會捲動。測試通道在舞台塞一顆右上角按鈕再 `scrollIntoView` 重現得出同樣畫面（舞台 scrollLeft 347）。
- 3 已做：Sol 方案審 1 輪無必改，照下方方案實作（含十語系提醒文案）。

## 已知限制
- 主題曲還沒抓到時回標題，繼續放當下的劇情配樂（主題曲抓到後自動換）；Sol 列必改、主線否決：少見，繼續放比突然靜音好。

## 第 3 項定案：作品包帶主角一句話
漏帶原因：一句話存在原桌 `vn.json` 的 `preferences.tagline`，包格式與匯入端都沒有對應欄位。
- **包格式**：`vn_work/format.rs` 的 `Manifest` 加 `tagline: String`（`#[serde(default, skip_serializing_if = "String::is_empty")]`）。放 manifest 不放 script：它跟桌名一樣是標題畫面的資料。`VERSION` 不升：manifest 已是 `serde(default)`、未知欄位忽略。
- **舊包相容**：舊包沒有這欄＝空字串＝標題不出這行（跟現在一樣）；新包給舊版 app 讀，舊版忽略這欄。
- **匯出**：`export.rs` 從 `snap.vn.preferences` 取 `tagline`、trim 後放進 manifest。重播包與附設定的包都帶。
- **匯入驗證與夾斷**（信任邊界）：`validate::check` 對 `manifest.tagline` 套桌名同一套 `clip_line`（換行改空白、夾到 `TITLE_CHARS`＝120 字）再 trim。畫面用 React 文字節點顯示，不會當 HTML 解讀。`probe_work` 不回這欄（確認框不需要）。
- **寫回**：`materialize.rs` 寫 `vn.json` 那段，tagline 不空就 `file.preferences.get_or_insert_with(Default::default).tagline = tagline`。重播、續寫兩種桌同一條路（附設定的包本來就不帶 `preferences`）。只有 tagline 的 `preferences`：`mood` 是 None，重播桌不抓音訊；續寫桌包內已有主題曲，不會因此重抓。
- **前端**：不用改。大廳 `WorldMeta.tagline` 已經從 `vn::player_tagline` 讀 `preferences.tagline`，`App.tsx` 再傳給標題畫面。
- **匯出前提醒文案**：`vnWorkExportWarn` 現在寫「玩家名與選項文字會原樣進包」，沒涵蓋這段文字。十語系已改成「玩家名、主角一句話與選項文字會原樣進包…」。
- **測試**（Rust）：匯出再匯入後一句話保留；含換行、超長的被夾成單行 120 字；舊包沒有這欄時沒有一句話。

## 下一步
- 測試通道已驗全案（截圖在 `<scratchpad>/vq3/FIX3/shots/`）；等主線讀 diff、送驗收。
