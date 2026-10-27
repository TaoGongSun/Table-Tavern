# web-mvu-memory-cap — 網頁版 MVU 引擎記憶體硬上限

Status: 已結案（2026-10-27 合併 main，Sol、Grok 驗收 DONE）

## Summary
MVU zod 卡的變數 schema 准在隔離環境執行（見 [mvu-zod-semantics](mvu-zod-semantics.md)），隔離要求含記憶體上限。網頁版原本只有時間上限，先當例外記待辦〔作者裁決 2026-10-27〕，本案補上。

## 定案（Sol、Grok 審查共識：方案 A＋A1）
- 網頁 zod 路徑改用 wasm 版 QuickJS（quickjs-emscripten-core、@jitl/quickjs-ng-wasmfile-release-sync，都鎖 `0.32.0`）跑桌面同一支 bundle；卡片只在 QuickJS 裡跑，原生載卡路徑全刪、不留退路。
- 硬上限是每支 Worker 自己的 `WebAssembly.Memory`（maximum 128 MB）；QuickJS `setMemoryLimit` 只是內層。wasm 以 base64 內嵌，主頁 CSP 不動，`'wasm-unsafe-eval'` 只加在隔離頁。
- 細節在規格 [plans/mvu-zod-semantics.md](../../plans/mvu-zod-semantics.md) 0、1.1、1.2、2.3、3、4.1、4.2 節。

## 已知限制
- WebKit 的文件本身不拿 CSP 管 wasm 編譯（主頁 eval 照擋；瀏覽器既有行為），e2e 只在 Chrome 驗主頁 wasm 被擋。
- `'wasm-unsafe-eval'` 要 Safari 16 以上。
- quickjs-ng 版本兩邊不同（網頁 0.12.1、桌面 0.16.2），靠 41 案＋三張真卡 golden 把關。
- 雙層 JSON 跳脫只多約 4–5 KB gzip，不改〔三方共識〕。
