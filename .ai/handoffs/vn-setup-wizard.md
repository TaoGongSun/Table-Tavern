# vn-setup-wizard — VN 開桌精靈：沒想法也能開桌、美術不等人、美術一覽

Status: 已合 main（2026-11-06，三包三筆）；等作者實機試玩（[實測佇列](../reference/verification-queue.md)第 30 項）。設計與審查紀錄見 [plans/vn-setup-wizard.md](../plans/vn-setup-wizard.md)。

## Summary
作者 2026-11-05 需求：只有一點概念的玩家也能靠題材勾選與選填讓 AI 產大綱與角色、改完再生成；立繪只生預設一張、願意可先挑、不願意直接開始背景補；劇情先生成再播；立繪與背景一覽可重生成。

## 案別與估計
大案，分三包：A 精靈（方案估 650）、B 時序（300，驗收）、C 美術一覽（300，背景上傳碰信任邊界，方案也送）。

## 作者裁決〔2026-11-05〕
維持串流照播；本案先做、合 main 後再試玩（實測佇列第 30 項順延）；同地點換時段用色調遮罩、專屬時段圖在一覽手動生成。

## 結案狀態
- 包 A 精靈（`src/features/vn/wizard/`、後端偏好與設定階段）：Sol 驗收 2 輪、Fable 截圖審，主線 verify 綠、截圖親看。
- 包 B 資產時序（暫定 `[bg]` 提早排隊、後端自動定稿 `vn_sprites/prepare.rs`、色調遮罩與白天基底）：Sol 2 輪、Fable 截圖審。
- 包 C 美術一覽（`src/features/vn/gallery/`、背景清單／上傳／取消）：Sol 2 輪、Fable 截圖審；結案整案審 3 條先修已做。
- **下一步：作者實機試玩**（實測佇列第 30 項，與 vn-mode、vn-ui-polish 一起）。試玩重點：從大廳「視覺小說」什麼都不填直接生成走完三步；精靈 ③ 不等就「進入遊戲」看標題畫面進度與舞台換圖；同地點換時段看遮罩；標題畫面「美術一覽」重生成、生成專屬圖、上傳替換；偏暗首幕背景下標題畫面可讀性（Fable 提的，通道只拍到夜景）。

## 包 B 已接／包 C 要接
- 暫定背景：`vn_backgrounds_prefetch(worldId, scene, source, text)`——`useVnCells` 串流中每多一行換行結尾的 `[bg]` 就送一次整段串流文字；後端只看換行結尾的行，接在正式格後補時段（`vn_backgrounds::streamed`），走同一個 `fill` 去重。
- 自動定稿：`vn_sprites::prepare`（命令 `vn_sprites_prepare(worldId, adopt)`，冪等、同桌一次一條）。③ 掛載時排一次；按開始時 `ArtStep` 先把「選了別張」的裝上，再骰中的角色放進 `adopt`（候選一生好就裝）。開工記定稿世代、裝前 `claim_base` 搶位、提交前再驗，玩家手動挑定／上傳／刪除就放棄。事件 `vn-sprite-status` 的 `expression: "base"`（queued／running／done／failed）；`vn_sprites_status` 併入排著、生成中與失敗的定稿（③ 重開、標題畫面靠它補讀）。跑的期間再被呼叫＝跑完再掃一輪，標題畫面「重試」就是再呼叫一次。設定已走完才接著補臉層。`sprite.json` 記 `candidate`（裝的是哪張候選；上傳＝無），③ 據此判斷「挑定這張」與按開始要不要裝，作品包匯出時拿掉。
- 舞台：立繪換新（卡圖／剪影→定稿、換定稿）延到換格才套用並交叉淡入 0.6s，臉層後到淡入 0.3s；夜晚對立繪疊半強度夜色（以身體圖為遮罩的 multiply 層）。
- 背景庫：每地點自動只生一張基底（時段清單第一項＝固定四時段的 day、卡自訂時段的第一個），`backgrounds.json` 每筆 `{name, base}`（舊桌只有地名照讀）。`BgSlot.mode`＝`Base`／`Regenerate`／`Dedicated`。
- 包 C 讀取：`vn_background_read(slug, period)` 回 `{src, period}`，period＝圖實際時段（≠要的＝基底，舞台疊遮罩）；`Library::base_period` 依「記下的基底 → 清單第一項 → 第一張有圖的」決定基底；`Library::shown` 是讀取順序。遮罩色在 `StageBackdrop.tsx` 的 `TINTS`／`tintOf`，一覽的遮罩預覽可直接重用。
- 生成專屬圖 `vn_background_generate_period(worldId, slug, period)`（以同地點既有圖當參考改光線）、重生成 `vn_background_regenerate(worldId, slug, period)`、取消排隊 `vn_background_cancel`（發 `cancelled`）：身分是 slug，地名由卡、`backgrounds.json` 或佇列工作補（`Library::target`）。完成事件 `vn-background-status` done 後，`useBackgrounds` 會重讀同 slug 所有用過的時段。
- 基底契約：`backgrounds.json` 記了基底時段就只認那張（還沒生好＝沒有基底，自動補照排）；地點還沒基底時寫入的任何圖都記「基底＝時段清單第一項」，非第一項的圖只當專屬圖。沒記錄的舊桌照舊退「第一張有圖的」。

## 包 C 已接
- 入口：`TitleScreen` 的「美術一覽」（結局一覽與設定之間）與 `gameMenuItems` 的 `gallery` 項，同一個 `gallery/ArtGallery`（`Panel` 加 `className`，底色不透）；遊戲選單原「重生成背景」項移除。縮圖點開 `ArtViewer` 燈箱（背景含遮罩、立繪可切已生好的表情），重播桌也能看。
- 立繪分頁 `SpriteGallery`：登場角色（`!archived && !stage`）一列，定稿＋各表情狀態點（`vn_sprites_status`＋事件；重播桌只畫已完成的），「編輯」掛既有 `SpriteTab`（`expressionsFirst` 表情列置頂；刪除立繪移到最下方 ghost）；換定稿下方註明表情會依新定稿重做。`SpriteBasePicker` 選的就是定稿時主鈕變灰並寫「已是定稿」。
- 背景分頁 `BackgroundGallery`：`vn_backgrounds_list` 回每地點 `{slug, location, periods, base, dedicated}`（卡宣告地點照卡上順序在前、沒圖也列；佇列中與失敗的地點也列；其餘依地名），圖與佇列狀態用舞台那份 `useBackgrounds`。排隊中可取消、失敗可重試（同一個動作）。
- 上傳 `vn_background_upload(worldId, slug, period, data)`：拒重播 → 鍵驗 → `normalize` → 持許可 `gallery::commit_upload`（重驗桌在、非重播、鍵；照基底契約記索引；取消同鍵排隊）→ 發 done → 地點缺基底就排基底。`backgrounds.json` 的讀→合併→寫回包在桌的短提交鎖（`state_commit::with_commit`）內：桌寫入許可是共用讀鎖、擋不住背景工作與上傳同時改索引。

## 已知限制
- 美術一覽上傳替換只取消同鍵「排隊中」的工作；正在生成的那張完成時會蓋掉剛上傳的圖（前端在該格生成中時停用上傳鈕，不是後端保證）。上傳選檔即覆蓋、沒有確認（本機原檔還在）。
- 取消只做背景排隊；立繪候選與自動定稿沒有現成的取消。
- 作品包匯入的桌沒帶 `backgrounds.json`：自由地點的圖在一覽顯示「未命名地點」、只能看（這類桌本來就是重播桌）。
- 卡自訂時段沒有語系標籤，一覽顯示卡上的時段鍵。
- 撰寫劇本途中關掉精靈又立刻點回來：聖經還沒寫回，會再呼叫一次生成（額度花兩次）。機率低，輕防。
- 自動定稿只在記憶體：重開 app 時在途的那位角色由標題畫面續排（`useArtProgress` 掛載時還缺定稿、也沒在排就呼叫一次 `vn_sprites_prepare`），精靈 ③ 也會再排。
- 按開始的瞬間候選剛好寫完、前端還沒收到回覆時，那張不會被採用（維持先前的定稿）。機率低。
- 舊桌與作品包匯入的桌：只有某時段圖的地點以那張當基底（例如只有夜景），其他時段疊遮罩在夜景上；新生成的地點不會。
- 暫定 `[bg]` 只比落檔早「該行之後到回覆結束」這段時間（實測 haiku 約 3 秒）；模型把 `[bg]` 寫在開頭時省得多，寫在結尾時幾乎沒差。
