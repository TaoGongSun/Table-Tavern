# vn-audio — VN 模式音訊（BGM／音效）

Status: 已合 main（2026-11-07）、分支已刪；Sol 方案審 2 輪＋驗收 2 輪 PASS；等作者實機試玩（實測佇列梯 3）。

## Summary
GM 依劇情（地點、氛圍、事件）自動挑 BGM 與音效，播放時自動切換。條件：免費、程式可接、非商用可授權；素材要能快取在玩家電腦、能隨 `.ttvn` 作品包再散布。署名另立 [vn-credits](../tasks/vn-credits.md)。

## 設計原則〔作者裁決 2026-11-07〕
- **分層**：第 0 層零設定＝BGM，所有玩家不填任何金鑰就有；第 1 層可選＝音效，玩家自填 Freesound token 才有。VN 氛圍九成靠 BGM，音效本來就稀疏，少了玩家幾乎不察覺；而音效每條來源都要玩家自填金鑰（目前玩家只填一把 OpenRouter 或走 CLI 零金鑰，為音訊多填幾把摩擦感明顯）。
- **沒金鑰不卡流程**：設定頁一格 Freesound token，說明一行「未填則沒有音效」；開桌、遊玩、匯出任何地方都不因缺金鑰停下或提示。
- **索引掛了照播**：`pieces.json` 是 Incompetech 網站自用資料檔、非文件化 API；索引與已下載曲子都快取本機，抓不到索引就用快取照常挑曲，不讓 BGM 全停。
- **v1 不接生成**：ElevenLabs、Stable Audio 等玩家真的要再說；Stable Audio 新帳號 25 credits 等於要玩家付費，不當預設。

## 來源調查（2026-11-07，agy＋Grok 搜尋，主線抓官方頁核對）

| 用途 | 來源 | 接法 | 授權與限制 |
|---|---|---|---|
| BGM（v1） | **Incompetech**（Kevin MacLeod，約 2000 首） | 免金鑰：`https://incompetech.com/music/royalty-free/pieces.json`（已抓到，含 `feel` 情緒、bpm、長度），本機比對挑曲 | CC BY 4.0，署名即可快取與隨作品包散布；單一作者風格，久玩會重複 |
| 音效、環境音（v1 可選） | **Freesound**（60 萬+） | 每位玩家自填自己的 API token（app 內建共用一把不行：API 條款 d) 禁止服務給未登入 Freesound 的使用者）；只抓 `preview-hq-mp3`，不必 OAuth2；60 次／分、2000 次／天 | API 限非商用；單檔各自 CC0／BY／BY-NC；只准必要的中間複本、禁整庫複製；長期快取界線未確認 |
| 曲庫沒有的音效（v1 不接） | **ElevenLabs Sound Effects**（玩家自填金鑰） | 一句描述生成，最長 30 秒；免費 10,000 credits／月（每秒扣點數兩份報告不一致，未確認） | 免費帳號不可商用；公開分享要把 `elevenlabs.io` 放作品標題（名單不算）；Eleven Music 禁止做成給第三人的曲庫，不用 |
| AI 生成 BGM（v1 不接） | **Stable Audio API**（玩家自填金鑰）或本機開放權重 | 一句描述生成；新帳號 25 credits 只夠試聽 | 官方知識庫稱可放進遊戲；本機權重 Community License（年營收未滿 100 萬美元） |

排除：
- **Jamendo**：API 條款（2013-09-17）禁止設計成快取或離線存取、Client ID 不得分享給第三方；網站授權頁又要求多媒體另買授權。
- **Pixabay**：音訊沒有 API，禁止單獨再散布；X 上有獨立遊戲作者因版權申訴跳過它的曲子。
- **Mixkit**：音樂排除遊戲。
- **Sonniss GDC、OpenTracks**：禁止交給第三人再用。
- **Suno／Udio**：沒有官方 API。
- **Mubert、Beatoven**：沒有免費方案。
- **MusicGen／AudioGen**：權重 CC BY-NC，產出權利不明，最多當備援。

素材規則：排除 CC ND；SA 會不會綁住整個作品包未確認，先排除；每檔記標題、作者、來源網址、授權。

## 結果
- 核心約 1540 行（`vn_audio.rs` 950：詞表、索引驗證與快取、挑曲、Freesound 搜尋與授權驗證、串流下載、.part→檔頭→改名、audio.json、每桌待辦集合）、測試約 400、i18n 60；立案估 250，差距來自信任邊界輸入各自成段（見 process-log）。
- 測試通道實測（主線親看）：進桌即落地主題曲（Incompetech「Daybreak」）與 audio.json；設定面板「靜音」勾選框與兩滑桿（放開音效滑桿試聽一聲）、偏好落檔；Freesound 格在 AI 連線整組之後；`parse_vn_text` 四種標記結果正確。「正在出聲」通道拿不到證據，留實機。

## 已知限制
- 只挑情緒不挑曲風（索引 `genre` 是數字代碼），古代場景可能配到電子樂。
- 音效不進作品包；匯入桌要自己的 Freesound 金鑰才有音效。
- 第一次播某情緒要等下載（5 分鐘曲約 12 MB），期間無聲。
- 沒有靜音快捷鍵，用設定面板的「靜音」勾選框。

## Next action
作者實機試玩（實測佇列梯 3）。規格與審查結論在 [plans/vn-audio.md](../plans/vn-audio.md)。
