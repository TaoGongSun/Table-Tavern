# OpenCC 繁→簡字元表（比對用）

- `TSCharacters.txt`：OpenCC 字典資料，取自 PyPI `opencc-python-reimplemented` 0.1.7 隨附的 `opencc/dictionary/TSCharacters.txt`，sha256 `6b5a0a799bea2bb22c001f635eaa3fc2904310f0c08addbff275477a80ecf09a`。固定此版本，不自動更新。
- 授權：Apache-2.0（`LICENSE.txt`），出處聲明見 `NOTICE.txt`。兩檔由 `tauri.conf.json` 的 `bundle.resources` 隨發佈物帶到資源目錄 `licenses/OpenCC/`（macOS 在 `Contents/Resources/licenses/OpenCC/`）。
- 用途：`glyph.rs` 只拿它把兩邊字串折成同一字形再比對，不轉換任何顯示或落檔文字。一字多候選取第一個（OpenCC 預設）。
