# grok-image-original-leftover — 方案（待審）

## 查證結論（grok 1.0.70）
- 每次 grok 單發都會在 `grok-home/sessions/<編碼 cwd>/<id>/` 留一份完整的 session 目錄。生圖那份約 488 KB：images/1.jpg 238 KB，另有 chat_history、updates、tool_definitions、terminal log。生圖 session 永遠不續聊。
- image_gen 的工具結果就是 `images/1.jpg` 的絕對路徑。模型可能把它轉存到 cli-workspace 再回報那條路徑，也可能直接回報 images/1.jpg。所以要等 app 讀完圖才能刪。
- grok 結束後直接刪 `<群組>/<id>` 是安全的：不會留下需要修的索引，`session_search.sqlite` 下次會自己清掉孤兒。`-s` 要合法 UUID（v4 可以），id 已存在時會拒絕。`grok sessions delete` 可能連網，不採用。
- grok 沒有不落地 session 的模式。`storage.cleanup_ttl_days` 是官方的閒置清理，但不在 GROK_CONFIG overlay 的白名單裡，要寫進 config.toml 才生效。這屬於定期清理，歸 agy-home-cleanup。

## 做法
1. 生圖改走 grok 時，image.rs 用 `lanes::new_session_id()` 產生 id，經 dispatch 新增的 `stream_image_via_transport` 傳下去；兩支公開串流函式簽名不變，共用私有的 `stream_turn_inner`，只有收到 id 的 grok 單發才帶 `-s <id>`。grok_home 用 `dispatch::grok_home_dir(&config_root(&app)?)`。
2. 圖寫進圖庫成功後，用 `grok_session::remove_session(grok_home, id)` 刪掉整個 session 目錄。寫入失敗或沒拿到圖就不刪，交給定期清理。
3. 不跟隨連結：`find_session_dirs` 的 sessions、群組、`<id>` 三層都只認真的目錄；群組裡沒有這個 id 就跳過。lane 的刪除與抹寫共用這支，一併受保護。
4. 找到 0 個目錄或 `remove_session` 回 Err 時只記一行 log，不影響生圖結果。

## 拍板
- 圖寫進圖庫後刪整份生圖 session 目錄〔作者裁決 2026-10-28〕。
- 生圖失敗（沒拿到圖、轉檔或寫圖庫失敗）不刪，交給定期清理〔作者裁決 2026-10-28〕。
- 結案前 rebase 到 grok-first-session-preamble-drop 之後（它先合 main）。

## 驗收
- 單元測試：生圖成功（模型直接回 session 內原圖路徑）整份刪、群組層不動；失敗不刪；三層符號連結都不跟隨，抹寫遇連結丟線且目標不動；略過沒有這個 id 的群組。
- 真生圖（grok 1.0.70、grok-4.6，測試 root）2 次：prompt_history 記的 session id 是 app 給的 v4 id；生成途中 `images/1.jpg` 落在該 id 底下，圖進圖庫後目錄已不在、工作目錄清空。

## 與 agy-home-cleanup 的分工
本案只在單次生圖成功後，即時刪掉那一個 session。定期清理按閒置時間掃整個家目錄，本案刪掉的目錄它不會再看到。兩邊同時刪同一個目錄時，晚到的一方遇到 NotFound 當成已刪。生圖進行中的 session 由定期清理用閒置期限避開。
