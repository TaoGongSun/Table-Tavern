# desktop-csp — 桌面版補 CSP

> **現況（2026-11-15）**：桌面版 CSP：macOS 已驗；剩 Windows 安全驗收。細項與「為什麼這台機器測不到」見 `reference/verification-queue.md`；本檔以下若還寫著其他「待實機／待部署／清單編號」，以這一行為準。

案別：小案，核心估 100–120 行（預算 100–140）（碰信任邊界，照流程上限做方案送審）。方案：[plans/desktop-csp.md](../plans/desktop-csp.md)。

## 現況
- 已進 main（Sol 方案審 2 輪、驗收 1 輪 PASS）：主視窗 CSP、`tt-sandbox` 外層＋內層 srcdoc、裁切框改靜態樣式、測試通道探測指令。macOS 測試通道正負面與正式 CSP 冒煙都過（見方案「施工結果」）。
- **Windows 安全驗收待測**：剩 Windows 實測（實測佇列梯 1 第 4i），過了就結案歸檔。
