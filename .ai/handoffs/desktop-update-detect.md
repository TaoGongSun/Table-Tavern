# desktop-update-detect — 桌面版 App 內更新與回退

分支：`desktop-update-detect`

## 現況
- 設計已全部拍板（2026-09-30），並經 Claude／Grok／Sol 三方審核達成共識。規格在 [plans/desktop-update-detect.md](../plans/desktop-update-detect.md)：方案 B 一鍵更新、App 內回退、每桌格式版本＋開到才轉、舊版遇新格式唯讀。
- Mac 只出 Apple Silicon：test-build.yml、README、CHANGELOG 已改。
- 還沒有程式。

## 下一步
- 先等 [ai-response-stop](../tasks/ai-response-stop.md) 完成（更新閘門要用它的中止功能），之後從包 1（發版管線）開始施工。包 2–5 首次公開時必須同版發出；計畫「實機驗證」各項要等有兩個真 release 才能測。
