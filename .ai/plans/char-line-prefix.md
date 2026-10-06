# char-line-prefix 做法

## 查證結論

- **來源是上下文示範**：送模型的歷史裡角色台詞一律帶「名字：」——API／codex 共線 assistant 訊息（`transport/assemble.rs` 台詞分支）、CLI 開線攤平（`transport/turns.rs` 歷史行、`cli/request.rs`）、claude／grok 共線回合後還把 session 檔最後一則 assistant 補成「名字：…」（`lanes/session_file.rs::prefix_last_assistant`、`lanes/grok_session.rs::rewrite_chat_history`）。模型看到自己過去每則回覆都以「名字：」開頭就照抄；回合尾段「不要加名字前綴」（`turns.rs` chars_lane_turn 結尾）壓不過。前綴是共線分辨說話者、快取前綴穩定的骨架，不能拿掉。
- **頻率與長相**：測試 root 兩桌 73 則角色台詞 26 則（36%）帶前綴，全部是「全名＋全形冒號」直接開頭（如「狐狸：狐狸擦杯子…」「林教授｜經濟學：林教授｜經濟學轉過身…」），沒看到半形、粗體、括號變體。名字含「｜」「・」也照全名寫。
- **落地路徑只有一條**：`commands/chat.rs::chat_with_character`（lane 與 API 兩支）回 `ChatReply.text` → 前端 `useChatController.replyOnce` 原樣 `appendEvent`；串流是 `push_delta` 逐段送 `on_delta`、前端疊進 `streamText`。中止時前端也照 `reply.text` 落一則 truncated。
- **lane 回聲比對**：`lanes::expected_reply_for` 拿回覆原文當預期，下一輪要跟逐字稿事件逐字相等，不等就 `ReplyDiverged` 撤線重開。所以逐字稿存剝過的字，預期回聲也必須是剝過的字。
- 角色卡沒有別名欄位，候選名只有 `card.name`。

## 做法：落地前剝除正典前綴（源頭與 session 抹寫都不改）

送審第 1、2 輪（Sol、Grok）意見已併入。

1. **剝除規則** `transport::strip_own_prefix(text, prefix) -> &str`（`messages.rs`，與 `speaker_prefix` 並列），`prefix` 一律是本輪角色的 `speaker_prefix(card.name, lang)`（繁中「名字：」、英文骨架「Name: 」含空格）：
   - 開頭字面 `starts_with(prefix)`、只剝一次、不用 regex（名字含「｜」等符號）。
   - 剝餘原樣保留、不吃後面的空白、落檔不 trim，保證 `prefix + 剝餘 == 原文 == session`。
   - 例外：剝餘全是空白（`char::is_whitespace`，含換行與空字串）就保留原文；重複前綴只剝一層（「狐狸：狐狸：正文」→「狐狸：正文」）。所以「逐字稿零前綴」不含這兩種情況。
   - 前導空白、另一種冒號、粗體、括號、大小寫一律不收、延後〔模型判斷·未裁決〕。取捨：收變體時 session 照吃原文會留下雙前綴（claude 前插成「名字：**名字**：…」並續聊示範下去；grok `matches(prefix) != 1` 撤線）；只剝正典字面時，模型原文本來就以正典前綴開頭，claude「沒前綴才補」與 grok `check_prefixed` 都不必動，session 內容也正好等於正典重建的「前綴＋剝餘」。觀測上 73 則中 26 則全是「全名＋全形冒號」，窄規則已涵蓋。Sol 提的「session 統一寫成 speaker_prefix＋strip(raw)」因此不做。窄規則仍分不出標頭與正文自述標籤，屬接受的取捨。
2. **回聲與回傳**：claude／grok 抹寫照舊吃原文，剝除只放在 `expected_reply_for` 與回傳，不就地改 `reply`。`ReplyEcho::Dialogue` 另帶 `prefix`（呼叫端用 `speaker_prefix(card.name, lang)` 組好），不從 `TurnInput.prefix` 反推——Agy 那個是 `None`，反推會讓逐字稿存剝餘、expected 存原文，每輪 `ReplyDiverged`。
3. **原文只轉換一次**：最終、中止、expected 各自從原文 strip 一次。API 的外層 `buffer` 照舊存原始增量（中止半截取它再 strip）；CLI 中止取本次 attempt 的 `TurnOutcome.text` 再 strip，不用跨降級重開的外層 buffer。避免「狐狸：狐狸：正文」中止時被剝兩次。
4. **串流過濾器**（`transport`，接在 `chat_with_character` 的 `emit` 與 `on_delta.send` 之間；buffer 仍收原文）契約：
   - 前綴辨識：累積字仍是前綴的開頭就扣住，最多前綴字數；第一個字對不上就整批放行並轉直通。
   - 空白扣留：前綴湊齊後，連續空白（`char::is_whitespace`，與 strip 的「全是空白」同一套）繼續扣，不設上限；看到第一個非空白字才丟前綴，放出扣住的空白＋正文，轉直通。
   - 完成與中止時仍未判定（前綴沒湊齊，或湊齊後全是空白）就把扣留原樣放行。
   - 結果保證：**單次 attempt 內**串流接起來 == `strip_own_prefix(該次全文)`。
   - `run_turn` 續聊失敗重開會沿用同一個 emit：過濾器每次 attempt 重建。
   - 例外：跨重試暫態殘字。前端 `useChatController` 串流只追加，第一試已吐字才失敗、重試再吐時，畫面會殘留舊字。這是既有行為，最終落檔以最終回覆為準；本案不加 attempt 重置訊號〔模型判斷·未裁決〕。
5. **測試**：
   - 規則：往返 `prefix + 剝餘 == 原文`；`Name:hello` 不剝；僅前綴、前綴＋空白不剝；近似名（「狐狸人：」）、重複前綴（只剝一層）、前導空白不剝。
   - 過濾器：在每個 Unicode 字元邊界切 delta 都等於 strip(全文)；`名字：\n正文`→`\n正文`；`名字：\n\n`→保留原文；前綴後大量空白再接正文；空白分段；完成與各位置中止都放行扣留。
   - lane：逐字稿存剝餘時下一輪 Resume；Agy（prefix None）仍 Resume；錯字仍 `ReplyDiverged`；grok 正典前綴抹寫仍過、內文再出現一次「名字：」仍撤線；私設抹除仍成立；受控 resume 失敗（第一試已吐字才失敗）後重試，最終文字與 expected 正確、本次 attempt 串流 == strip(本次全文)。
6. **實測**（新開桌，只斷言新落的句）：測試通道、claude 預設 Sonnet、兩角色桌約 8 輪；API 共線（OpenRouter 免費模型）3–4 輪；grok live 幾輪驗雙檔抹寫。檢查新句零前綴（例外見第 1 點）、串流不閃前綴、lanes 不因 `ReplyDiverged` 重開。另做串流→完成的視覺檢查（`狐狸：\n正文`、多空行、縮排正文）：串流時名牌下多一行空白（`.message` 為 pre-wrap）、落檔後經 Markdown 消失，屬已知。

## 實測結果（2026-10-07，測試通道，新開範例桌 01M49J6S154YAXCSBYVCDCFZSK）

- claude（sonnet）8 輪：新句 8/8 無前綴；同一條 session 續聊到 sent 16，reply-diverged 0；session 檔每則 assistant 都只有一層「名字：」。串流觀察到第 5、6 輪第一段只放出「狐」「騎士」，證明模型當輪寫了前綴並在串流中被剝掉；任何串流開頭都沒有出現「名字：」。
- API 共線（OpenRouter `nvidia/nemotron-3-super-120b-a12b:free`）4 輪：新句 4/4 無前綴，串流開頭乾淨。測試 root 原本的 `stealth/space-bunny-alpha` 已下架（404），gemma 免費版被上游限流（429），inkling 免費版限定 agent 應用（403），所以換成 nemotron。
- grok（grok-4.5）4 輪：同一條線續聊到 sent 32，不撤線、reply-diverged 0；chat_history 與 updates 兩檔每則回覆都只有一層前綴。
- 落檔後的視覺：「\n正文」「\n\n正文」「  縮排正文」都渲染成 `<p>正文</p>`，名牌下沒有多出空行。

## 不做〔模型判斷·未裁決〕

- 不改歷史呈現（拿掉或換掉 assistant 的「名字：」）：共線靠它分辨說話者，改了整條快取前綴失效；剝除後正典歷史固定是單一前綴，「名字：名字：」雙前綴示範不再累積。
- 不回頭清既有逐字稿的舊前綴（發佈前舊桌不顧）。
- 不處理模型替別的角色加前綴、GM 旁白前綴（GM 走旁白剝狀態欄，收尾句已要求不加）。
