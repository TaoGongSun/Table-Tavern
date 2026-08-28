# 拆檔驗收工具

把一個大 `.rs` 檔按領域拆進同名資料夾時，用這五支證明「production body 逐 byte 未動、對外路徑一條沒少」。
已用於 `data-split`（data.rs 5768 行 → data/ 九檔）。

## 跑法

拆之前，先留一份原檔並抓兩份基準：

```bash
cp src-tauri/src/big.rs /tmp/big.rs.orig
python3 scripts/split-verify/slice_items.py /tmp/before src-tauri/src/big.rs
python3 scripts/split-verify/pub_api.py src-tauri/src/big.rs > /tmp/before-pubapi.txt
cargo test --lib -- --list > /tmp/before-tests.txt
```

拆完之後比對：

```bash
# 前兩支要排除測試夾具檔；命令替換直接內嵌，不要先存成變數
# （zsh 不對未加引號的 $var 做斷詞，整串會被當成單一檔名）
prod() { ls src-tauri/src/big/*.rs | grep -v test_support; }
python3 scripts/split-verify/compare.py /tmp/big.rs.orig $(prod)
python3 scripts/split-verify/pub_api.py $(prod) | diff /tmp/before-pubapi.txt -
python3 scripts/split-verify/facade_check.py /tmp/big.rs.orig src-tauri/src/big/mod.rs
python3 scripts/split-verify/test_bodies.py /tmp/big.rs.orig src-tauri/src/big/*.rs
```

`test_bodies.py` 是唯一要含 `test_support.rs` 的（測試都在各檔的 `mod tests` 裡，夾具檔沒有
`#[test]`，含進來也不影響）。`pub_api.py` 的 diff 不會是空的——你刻意升的每個 `pub(super)`
都會多出一行，確認多出來的正好是那幾條就對了。

`compare.py` 會印出四個數字：遺失、多出、可見度變更、內容變更（附 diff）。
理想結果是前兩項為 0，第三項只有你刻意升級的跨檔私有 fn，第四項為 0。

## 五支各做什麼

- `slice_items.py` — 把檔案切成頂層 item 逐個落檔＋MANIFEST。切完會 assert
  「所有切片串接後逐 byte 等於原檔」，切割本身沒有遺漏或重疊才會往下跑。
  `#[cfg(test)] mod tests` 不算 production，自動跳過。
- `pub_api.py` — 抽 `pub` 項目的正規化簽名（含 struct 欄位與 impl 方法），排序輸出。
  它刻意跳過 `pub use`，所以證明的是「定義都還在、簽名一個字沒變」，不是對外路徑——
  那是 `facade_check.py` 的事。
- `compare.py` — 前後比對。每個 item 算兩個 hash：原文一份、剝掉可見度前綴一份，
  於是「只升了 `pub(super)`」和「body 真的被改了」分得開。
- `facade_check.py` — 拆前的頂層 `pub` 項目，`mod.rs` 有沒有全部供得出來（漏掉／多出／
  可見度改變各印一行，有任一項就 exit 1）。`pub_api.py` 刻意跳過 `pub use`，證明的是
  「定義都還在、簽名沒變」；對外路徑接沒接回來是另一回事，要靠這支。編譯只覆蓋得到
  有人呼叫的路徑，沒人叫的 `pub` 項目漏了 re-export 不會報錯。
- `test_bodies.py` — 每支測試的 body 逐 byte 比對。`cargo test -- --list` 的名單只證明
  「測試還在、名字沒變」；斷言被拿掉、assert 改寬鬆都不會改名字，要靠這支才看得到。

## 注意

- `pub_api.py` 靠檔內的 `#[cfg(test)]` 判斷測試區塊。測試專用檔（如 `test_support.rs`
  整檔靠 `mod.rs` 的 `#[cfg(test)] mod` 掛載）不要放進 after 清單，否則夾具簽名會混進來。
- 切割規則假設程式碼是 rustfmt 風格：頂層 item 結束於第 0 欄的 `}`／`];`／`);`，
  且該行不以 `{`、`[`、`(`、`,` 結尾（那是多行簽名的續行）。
- 這五支都比不到的：`use` 有沒有綁錯（同名型別誤指到另一個模組），以及非 host target
  的編譯（macOS 跑 `cargo check` 不會編到 Windows 分支）。前者要人工看 import，
  後者要 CI。
