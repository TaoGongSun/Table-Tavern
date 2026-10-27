# web-mvu-memory-cap — 網頁版 MVU 引擎記憶體硬上限

## Summary
MVU zod 卡的變數 schema 准在隔離環境執行（見 [mvu-zod-semantics](mvu-zod-semantics.md)），隔離要求含記憶體上限。網頁版 Worker 目前只有時間上限，達不到記憶體上限這一條。先當例外、記待辦〔作者裁決 2026-10-27〕。

## 已知線索
- 可考慮 quickjs-emscripten（WASM 版 QuickJS，可設記憶體上限）。桌面版是否已有記憶體上限未查。
- 尚未評估體積、效能與和現有 Worker 路徑的整合成本。

## Next action
排在 mvu-zod-semantics 合併之後；開工先短驗證 quickjs-emscripten 能否設硬上限並跑現有 zod 卡。

## 驗收方向
惡意或失控的 schema（無限配置）在網頁版被記憶體上限終止、不拖垮分頁；正常 zod 卡行為不變。
