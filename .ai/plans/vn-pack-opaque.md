# vn-pack-opaque — 作品包不透明容器＋音效進包

交接：[handoffs/vn-pack-opaque.md](../handoffs/vn-pack-opaque.md)。中案（核心估 150–170）；匯入契約是信任邊界，方案送 Sol 一審。

## 0. 現況（2026-11-07 查）
- 寫：`vn_work/export.rs::write_pack` 用 `ZipWriter<BufWriter<File>>`，先寫 `<path>.part` 再改名。讀：`vn_work/archive.rs::Pack::open` 先 `check_directory` 串流掃 EOCD，再 `ZipArchive<BufReader<File>>`；條目名只當查表鍵。
- 音效被擋在三處：`format.rs::Asset::parse` 只認 `assets/audio/bgm-<key>.mp3`；`export.rs::write_audio` 只掃 `bgm-`；`vn_audio::{export_entries, import_entries}` 只留 kind `bgm`。`vn_audio::file_key` 已認 `bgm-`／`sfx-` 兩種檔名。
- 重播桌播音效：`useVnAudio` 從 `vn_audio_read` 列出的檔找 `sfx-<key>`，key 由文字裡的 `[sfx: …]` 重新算（SHA-256 前 16 hex，合 `valid_key`），與 BGM 同一條路，前端不用改。
- `is_replay` 的桌不抓音訊（`vn_audio.rs` 四處），匯入的音效只靠包內的。

## 1. 容器（新檔 `vn_work/opaque.rs`，估 90 行）
- 檔頭 16 位元組：`b"TTVNPK\x01\x00"`＋8 位元組 seed（匯出時取系統時間奈秒經 splitmix64）。之後是整個 zip 串流逐位元組 XOR keystream。
- keystream：第 p 個 payload 位元組用 `splitmix64(seed ^ (p >> 3))` 的小端第 `p & 7` 個位元組。位置可直接算，所以 seek 不用重跑。目的只是讓雙擊、解壓軟體認不出 zip（作者裁決：一般人打不開就好，特地破解的不管），不是加密，不引新 crate。
- 一個型別 `Opaque<T>` 同時當讀寫轉接器：持 inner、`offset`（檔頭長；明文模式 0）、`seed: Option<u64>`（None＝明文不 XOR）、`pos`（payload 位置）。`Read`／`Write` 做 XOR 並推 `pos`；寫入端不改呼叫者的 `&[u8]`，固定 8 KiB 小塊複製後 XOR 再寫，不為整首 mp3 配暫存。
- Seek 契約（`ZipArchive::new` 靠 `End` 找 EOCD、`ZipWriter` 每檔寫完 seek 回去補 CRC 與大小，兩邊都靠 `pos` 對齊 keystream）：三種 `SeekFrom` 一律先 `inner.seek`（`Start(x)` 先加 `offset`），拿回的絕對位置減 `offset` 存進 `pos`，不自己累加；結果落在檔頭內回 `InvalidInput`；`stream_position` 回 payload 相對位置。
- `opaque::create(file) -> io::Result<Opaque<File>>`：寫檔頭、回轉接器。`opaque::open(file) -> io::Result<Opaque<File>>`：讀前 16 位元組（`read` 可能讀不滿，用 `read_exact` 並接住 `UnexpectedEof`），magic 對＝不透明模式，不對＝明確 `seek(Start(0))` 走明文模式（舊明文 zip 照收；檔太短同樣走明文，之後 zip 解析失敗回「不是作品檔」）。
- `export.rs::write_pack`：`ZipWriter::new(BufWriter::new(opaque::create(file)?))`；`finish().into_inner()` 後再 `.into_inner()` 取回 `File` 做 `sync_all`。
- `archive.rs::Pack`：`archive: ZipArchive<BufReader<Opaque<File>>>`；`open` 先 `opaque::open`，`check_directory` 改收 `impl Read + Seek`，在轉接器上掃（掃到的是解碼後的 zip）。`pack_bytes` 上限含檔頭，不另扣。
- manifest `VERSION` 不動：內容格式沒變；舊版 app 開新包得到「這不是 VN 作品檔」，發佈前可接受〔模型判斷·未裁決〕。

## 2. 音效進包（估 70 行）
- `format.rs`：`Asset::Audio { key }` → `Asset::Audio { kind: AudioKind, key }`，`enum AudioKind { Bgm, Sfx }` 帶 `prefix()`（`"bgm"`／`"sfx"`）；`path()` 出 `assets/audio/<kind>-<key>.mp3`，`parse` 認兩種前綴。key 白名單分開：Bgm 維持 `vn_audio::valid_key`；Sfx 的 key 只會是 SHA-256 前 16 hex（`parse_sfx`），`parse` 與 `import_entries` 對 Sfx 一律收 `^[0-9a-f]{16}$`（新 `vn_audio::valid_sfx_key`，匯入契約精確白名單）。
- `LIMITS.audio_files` 200 → 1000（bgm＋sfx 合計）：bgm 的 key 空間本身是 19 情緒單選＋兩兩組合＋theme 近 190，共用 200 等於音效沒有額度；音效一格一檔、以 query 去重，長篇可能破 200。`entries: 4096` 餘裕夠（2000 圖＋1000 音訊＋雜項）；單檔 `audio_bytes` 32 MB 與 `total_bytes` 1.5 GB 不變，實際大小仍由它們管。
- `export.rs::write_audio`：改用 `vn_audio::file_key` 認檔（改 `pub(crate)`），收 `(kind, key)` 對；`export_entries(dir, &pairs)`。
- `vn_audio.rs`：`export_entries` 與 `import_entries` 的 `keys` 改成 `BTreeSet<(String, String)>`（kind, key），kind 只准 `bgm`／`sfx`；`import_entries` 的 200 筆硬上限改收參數，呼叫端傳 `limits.audio_files`。其餘契約不變：key 白名單、字串夾斷、對不到已落地檔的筆丟掉、落檔路徑由 kind＋key 重組。
- `materialize.rs`：`audio_keys: BTreeSet<(String, String)>`；`target` 的 `Asset::Audio` 出 `<kind>-<key>.mp3`。mp3 檔頭檢查、`audio_bytes` 上限照舊套在 sfx 上。
- 前端、i18n 不動；感謝名單讀同一份 audio.json，音效匯入後自動列入。

## 3. 測試（估 ≤ 核心 1 倍）
- `archive_tests.rs`：既有 `pack()` 寫明文 zip，所有舊測試即是「舊明文包照收」；加：不透明包開得了並讀得到條目；magic 對但 payload 是垃圾 → 「不是作品檔」；檔只有 3 位元組 → 「不是作品檔」。
- `tests.rs`：`entries()` 與壞 mp3 測試改經 `opaque::open`／`opaque::create` 讀寫；`export_import_keeps_bgm_and_drops_sfx` 改成帶 bgm＋sfx（manifest 列兩個 mp3、匯入後 `sfx-<key>.mp3` 落地、audio.json 含 sfx 筆）；`audio_asset_names_are_whitelist_only` 的 sfx 斷言反轉、加 `sfx-Nope` 拒收；加：匯出檔前 8 位元組是 magic 且 `zip::ZipArchive::new(File)` 直接開會失敗。
- `vn_audio_tests.rs`：`import_drops_sfx_and_unknown_keys…` 改成 sfx 對到檔就留、對不到就丟。

## 4. 驗收
`npm run verify` 17 步綠；Sol 方案審 1 輪（匯入契約）＋驗收 1 輪。介面行為沒動，依 CLAUDE.md「測試通道只在動到介面行為時跑」不跑測試通道、不拍截圖。端到端證據鏈：cargo 測試「匯出 bgm＋sfx → 匯入 → `sfx-<key>.mp3` 落地、audio.json 含 sfx 筆」＋ vn-credits 既有的 ArtGallery 音效段 vitest（名單讀同一份 audio.json）。

## 5. 常識審（主線 Fable 自做）
玩家看得到的差別只有兩件：作品包雙擊打不開（這是目的），重播桌有音效且名單列音效作者。匯出對話框不加字〔作者裁決 2026-11-07，vn-credits §4〕。

## 結果（2026-11-07）
Sol 方案審 1 輪無必改；Grok 施工 1 包；主線小修四處（`xor` 每 8 位元組算一次、`index_keeps` 用 `contains`、`write_audio` 改用 `Asset::parse` 認檔名、vn-audio.md §4 整段改寫）；verify 17 步綠（cargo 2735）；Sol 驗收 1 輪無必改（另自寫暫存測試驗跨區塊覆寫、短寫、非法 Seek 復位 3／3）。核心約 270 行（估 150–170），測試 76。

## 已知限制
- 容器只擋一般人：XOR keystream 可由原始碼還原；匯入後 mp3 明文落在對方資料夾。授權綁帳號的素材不因此可用。
- 舊版 app 開新包回「不是作品檔」而不是「版本太新」。

## 觀察
- 無。
