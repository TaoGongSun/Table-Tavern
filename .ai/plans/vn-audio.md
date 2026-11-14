# vn-audio 方案（中案，核心估約 250；碰信任邊界輸入與鎖，方案送 Sol 一審）

立案與來源調查見 [交接檔](../handoffs/vn-audio.md)。本檔是施工規格；拍板條目標〔作者裁決〕，其餘為主線設計〔模型判斷·未裁決〕。常識審由主線（Fable）直接做〔作者裁決 2026-11-07〕，結論已折進各節，摘要在 §6。

## 1. GM 標記（一處標記、兩種素材）

沿用 `[bg]` 的寫法，提示詞加在 `transport/vn_format.rs` 的場景那條後面，中英各一條；解析加在 `chat_turn/vn_script.rs` 的 `marker()`／`parse_cells`，型別加進 `VnCell`（Rust 與 `backend-contracts.ts`）。

- **BGM**：`[bgm: 情緒]` 單獨一行，情緒只能用固定詞表（Incompetech `feel` 的 19 個詞，英文原字）：`Dark, Grooving, Relaxed, Bright, Calming, Bouncy, Driving, Mysterious, Intense, Eerie, Unnerving, Somber, Mystical, Uplifting, Humorous, Epic, Action, Suspenseful, Aggressive`；可寫兩個用逗號分隔（交集，沒交集退回第一個）；`[bgm: off]`＝靜音。只在氣氛改變時寫，通常緊接 `[bg]`；不寫＝沿用。提示詞附一行情境對照讓 GM 選詞：戰鬥→Action／Intense／Aggressive、懸疑→Mysterious／Suspenseful／Eerie、悲傷→Somber／Dark、日常→Bouncy／Relaxed／Grooving、溫情戀愛→Calming／Bright／Uplifting、壯闊→Epic、搞笑→Humorous。格：`{ type: "bgm"; moods: string[] }`（`off`＝空陣列）。詞不在表上的整格丟掉（當沒寫）。
- **SFX**：`[sfx: 英文短描述]` 單獨一行，例如 `[sfx: door creak]`、`[sfx: thunder]`，播到這格時放一次；每回合最多兩個。格：`{ type: "sfx"; query: string; key: string }`，`key`＝query 正規化（小寫、去多餘空白）後 SHA-256 前 16 hex，與背景 slug 同法。
- 角色線 `filter_character_reply` 一併剝掉這兩種標記。

## 2. 後端挑曲與快取（`src-tauri/src/vn_audio.rs` ＋ `commands/vn_audio.rs`）

照 `vn_backgrounds.rs` 的「fill 排隊→背景下載→落檔→發事件」模式，但不生圖、只下載。

- **索引**：`pieces.json` 快取在 `<config_root>/cache/incompetech-pieces.json`；超過 7 天才重抓，抓不到就用舊檔；兩者都沒有＝這次挑不到曲，不報錯（下次 fill 再試）。只用 `title`、`filename`、`feel`、`length`。**驗證**（信任邊界）：回應上限 4 MB、必須是陣列、每筆 `title`／`filename`／`feel` 為字串；`filename` 只准單一檔名（非空、不含 `/`、`\`、`..`、控制字元、≤200 字元、以 `.mp3` 結尾），不合格的筆跳過；整份解析驗過才寫暫存檔（隨機後綴，避免跨桌互撞）再原子改名蓋快取，失敗保留舊檔。曲子 URL 固定 `https://incompetech.com/music/royalty-free/mp3-royaltyfree/<percent-encoded filename>`。
- **挑曲**：候選＝`feel` 含所有指定情緒的曲子（沒交集退回第一個情緒；再沒有＝跳過）；一桌一情緒固定一首：`hash(world_id + moods) % 候選數`，不另存狀態，同桌同情緒永遠同一首（真 VN 就是一景一曲，玩家會記住主題）。只挑情緒不挑曲風（索引的 `genre` 是數字代碼、沒有對照表），列已知限制。
- **主題曲**：每桌一首，情緒由精靈 ① 的 `preferences.mood` 對映（light→Bright、warm→Calming、dark→Mysterious、沒選→Calming），key 為 `theme`；標題畫面一律播它，按開始／繼續後才淡入當下那格的配樂；遊玩中本幕還沒寫過 `[bgm]` 也播它（VN 慣例，作品桌與原桌一致）〔模型判斷·未裁決〕。主題曲還沒落地時標題維持現在這首（新開桌＝無聲）。建桌成功後精靈與一鍵流程呼叫 `vn_audio_fill(world_id, null)` 跟背景一起排，標題畫面出現前多半已落地。
- **音效**：只在 `api_keys["freesound"]` 有值時做；`GET https://freesound.org/apiv2/search/text/?query=<q>&fields=id,name,previews,license,username,url,duration&filter=duration:[0 TO 15] license:("Creative Commons 0" OR "Attribution")&page_size=5&sort=rating_desc`，`Authorization: Token <key>`；取第一筆的 `previews["preview-hq-mp3"]`。`filter` 與 license 值名稱是依文件推的，施工時對 https://freesound.org/docs/api/resources_apiv2.html 核對再寫。沒結果＝跳過，不報錯。排除 BY-NC、ND、SA〔交接檔素材規則〕。**驗證**：回應上限 256 KB、`results` 為陣列；逐筆檢查 `license` 回應值確實是 CC0 或 CC BY（比對授權 URL，不只信 filter）、`previews["preview-hq-mp3"]` 是 `https://` 且主機為 `freesound.org` 或 `cdn.freesound.org`，不合格的筆跳過取下一筆；token 只送搜尋 API，抓 preview 不帶 token；下載不跟隨跨主機重新導向。
- **落檔**：`worlds/<id>/vn/audio/bgm-<key>.mp3`、`sfx-<key>.mp3`（bgm key＝情緒詞小寫用 `+` 串；主題曲 `bgm-theme.mp3`；檔名一律由 kind＋key 推得，不另存），下載走 `generated_image.rs` 的 `download` 同一套逾時與位元組上限（bgm 上限 32 MB、sfx 4 MB），先寫 `<檔名>.part` 再改名（照 `vn_sprites/matting.rs`）；改名前驗 mp3 檔頭（≥4 bytes；`ID3` 開頭且版本位元組 ≤4，或 MPEG frame sync：前 11 位全 1、layer 位元 ≠ 00、bitrate index ≠ 0xF），不合格刪 `.part` 當失敗。索引 `worlds/<id>/vn/audio/audio.json`：每筆 `{ kind, key, title, author, source_url, license }`（給 vn-credits 用），用 `commit_world_write_atomic` 寫。
- **指令**：`vn_audio_fill(world_id, scene)` 掃該幕格；`vn_audio_read(world_id)` 回 audio.json 與各檔絕對路徑（前端轉 asset URL）。**去重與鎖**：照 `vn_backgrounds` 的待辦集合模式——每桌一個待辦集合（key 去重），fill 與 prefetch 都只是把 key 丟進集合，忙碌時不略過、工作迴圈做到集合空為止；網路等待期間不持任何世界鎖；落地前取世界寫入許可並重驗桌仍存在，rename 成功、audio.json 提交後才發 `vn-audio-status`，失敗清 `.part`。**哪些桌抓**：重播桌（replay）一律不抓；其他桌（含匯入後可續寫的桌）缺什麼抓什麼，sfx 仍以有 token 為前提〔模型判斷·未裁決〕。
- 串流中寫完的 `[bgm]`／`[sfx]` 行比照 `vn_backgrounds_prefetch` 提早排（`useVnCells.ts`）。

## 3. 前端播放（`src/features/vn/audio/`：`useVnAudio.ts`＋設定項）

- `stageAt` 摺疊出 `stage.bgm: string[] | null`（null＝從沒寫過、`[]`＝off）；`useVnAudio(worldId, stage.bgm, currentCell)`：
  - BGM：一個 `<audio loop>`；情緒鍵變了就 1 秒淡出舊曲、淡入新曲（兩個 element 交替或音量斜坡擇一，簡單為先）；檔還沒落地就等 `vn-audio-status`。
  - SFX：播到 `sfx` 格時 `new Audio(url).play()` 一次；同一格回看不重播：記「已播集合」（鍵＝事件 id＋格序號），切桌（worldId 變）才重置；從章節重玩產生的是新事件，自然會再播。
  - 章末字卡、完結畫面沿用當前 BGM，標題畫面改播主題曲（見 §2）〔模型判斷·未裁決〕；離開 VN 播放器時停。
  - 瀏覽器自動播放限制：第一次使用者互動後才開始（Tauri WebView 一般允許，失敗就 catch 靜默，下一次互動再試）。
- **讀檔方式**：標準做法是 Tauri asset protocol（`convertFileSrc`）；現在沒接線，要做三件事：`Cargo.toml` 的 tauri 加 `protocol-asset` feature、`tauri.conf.json` 的 `app.security.assetProtocol.enable: true`（scope 留空、執行期用 `app.asset_protocol_scope().allow_directory(<audio 目錄>, false)` 只開 `worlds/*/vn/audio/`）、CSP 加 `media-src asset: http://asset.localhost`。可串流、可 seek、不用把 12 MB 搬過 IPC。實作者若判斷 IPC 回 bytes＋`blob:` 更簡單可改（CSP 改加 `media-src blob:`），但要寫明理由。
- **設定**：VN 設定面板加「音樂音量」「音效音量」兩個 `<input type="range">`（0–100，預設 70／100），偏好鍵 `vn_bgm_volume`、`vn_sfx_volume`，走 `changePreference`，與打字速度同一套。
- **快轉防護**〔作者裁決 2026-11-07〕：skip 開著時不播音效也不登記已播（跳過去的格不會再被到達，停下那格旁邊的照常響）；同時最多兩聲在響、兩聲起播至少隔 250 毫秒，超過就丟；提示詞加一句「音效只配明確的單次事件（開門、雷聲、碎裂），不配每句台詞、不配情緒」。理由：skip 每格 80 毫秒，重開桌後已播集合是空的，從頭 skip 會以一回合兩聲連發。
- **靜音與試聽**〔作者裁決 2026-11-07〕：設定面板兩滑桿上方加「靜音」勾選框（偏好 `vn_muted`，音樂音效都靜、滑桿值保留）；音效滑桿放開時用桌裡已落地的任一音效以新音量放一聲試聽（沒有落地的就不放、靜音時不放）；音樂滑桿不試聽，標題畫面本來就在播。
- **Freesound token**：設定視窗 `SettingsForm.tsx` 金鑰區加一格 `api_keys["freesound"]`（`type="password"`），一行說明「未填則沒有音效；BGM 不需要」，附 Freesound API 金鑰頁連結。任何流程不因為沒填而停或提示。

## 4. 作品包

`vn_work/format.rs` 的 `Asset` 有 `Audio { kind, key }`（路徑 `assets/audio/<kind>-<key>.mp3`；bgm 鍵白名單 `^[a-z0-9+]{1,64}$`、sfx 鍵 16 個小寫 hex；副檔名由型別決定、不從索引拿）與 `AudioIndex`（`assets/audio/audio.json`，走 `json_bytes`）；`export.rs` 掃 `vn/audio/` 的 bgm 與 sfx、`materialize.rs` 寫回；`LIMITS` 有 `audio_bytes: 32 << 20`（每檔）與 `audio_files: 1000`（兩種合計、實際計數），總量沿用 `total_bytes`。**匯入契約**：audio.json 最多 `audio_files` 筆，每筆 `kind` 只准 `bgm`／`sfx`、`key` 合該種白名單、字串欄位夾斷（title／author ≤200、source_url／license ≤500）、每筆必須對到包內已驗過的同種 mp3，對不到的筆丟掉；寫回與取 URL 都用 kind＋key 重組路徑，不信索引字串。mp3 進包與匯入都做同一套檔頭檢查（§2），不轉碼。容器與音效進包的裁決見 [vn-pack-opaque](vn-pack-opaque.md)。

## 5. 驗收（Sol 一輪）

- 標記：中英提示詞各一條；`[bgm: Mysterious, Dark]`、`[bgm: off]`、詞不在表上、`[sfx: door creak]` 四種解析測試；角色線剝標記。
- 挑曲：同桌同情緒同一首、交集退回、索引抓不到用快取；Freesound 無 token 不呼叫、有 token 無結果不報錯。
- 播放：換情緒淡入淡出、off 停、sfx 一格一次、離開播放器停；音量設定立即生效並落偏好。
- 作品包：匯出含 audio 與 audio.json、匯入後能播；非白名單檔名拒收。
- 測試通道：標題畫面＋遊戲選單設定面板兩張截圖（音量項）；BGM 實際播放靠 `invoke vn_audio_fill` 後檢查檔案落地與 audio.json，不靠聽。真 GM 回合會用作者額度，用預寫好 `[bgm]`／`[sfx]` 格的桌測；音效實測要作者自己的 Freesound token，沒有就只靠單元測試。
- 完整 verify 17 步綠；grep 帳號名乾淨。

## 6. 常識審（主線 Fable，設計開工前）
- 一桌一情緒固定一首：對，VN 玩家靠重複聽到的主題記住場景；兩個詞的組合自然帶出變化。
- 19 詞夠用，但 GM 要有情境對照才不會亂選 → 已加進 §1 提示詞。
- 新桌標題畫面本來會沒聲音（還沒有 `[bgm]` 格）→ 加主題曲（§2），建桌就排。
- Freesound 說明文案要讓玩家一眼看出「選填、不填也有音樂」→ §3 用「選填」開頭。
- 作者看截圖後裁決：加整體「靜音」勾選框（拉到 0 會丟掉原本的音量位置）、音效滑桿放開時放一聲試聽（音效在遊玩中才出現，不試聽不知道拉到哪）；靜音快捷鍵不做；曲風不挑是已知限制；首次下載幾秒的空白不提示。

## 已知限制
- 只挑情緒不挑曲風，古代場景可能配到電子樂。
- 音效隨包帶走（vn-pack-opaque）。
- 第一次播某情緒要等下載（5 分鐘曲約 12 MB），期間無聲。

## 觀察（範圍外，只記一行）
- 測試通道拿不到「正在出聲」的證據：Audio 物件不在 DOM、resource timing 不記媒體請求；下次要驗播放本身得在 hook 掛測試用鉤子，或留實機。
