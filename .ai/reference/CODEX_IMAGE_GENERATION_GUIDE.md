# Codex CLI 生圖方案：給 Claude 的研究交接

> 用途：讓 Claude 整理各家 AI CLI 的生圖能力與安裝需求。  
> 最後核對：2026-07-27。能力、模型與方案限制可能改變，實作前應再查官方文件。  
> 重要：不要假設目標使用者的電腦已安裝 Codex、Image Gen、Node.js、API CLI 或任何 SDK。

## 直接結論

Codex CLI 可以透過內建的 `$imagegen` skill 生圖。

目標電腦必須安裝：

1. Node.js 與 npm，或其他官方支援的 Codex CLI 安裝前置環境。
2. 官方 Codex CLI：`@openai/codex`。
3. 完成 `codex login` 認證。

不必另外安裝一個名為 Image Gen 的 npm、pip 套件或第三方 plugin。`imagegen` 是目前 Codex 隨附的內建 system skill。若安裝最新版 Codex 後仍沒有 `$imagegen`，應視為版本、session 載入、帳號方案或工作區政策問題，而不是自動下載同名第三方套件。

內建生圖路徑不需要 `OPENAI_API_KEY`；它使用 Codex／ChatGPT 登入並計入一般 Codex 使用額度。目前官方文件指出內建生圖使用 `gpt-image-2`。

## Claude 應記錄的安裝規格

### 必要安裝

若使用 npm 安裝：

```bash
npm install --global @openai/codex@latest
```

安裝驗證：

```bash
codex --version
```

登入：

```bash
codex login
```

診斷：

```bash
codex doctor
```

如果 `codex` 找不到，先處理 Node.js、npm 全域安裝目錄或 `PATH`。不要用非官方同名套件替代。

### Image Gen 是否需要另外安裝

不需要。

安裝或更新 Codex 後，啟動一個新的 CLI session：

```bash
codex
```

在 session 中執行：

```text
/skills
```

確認清單中有 `imagegen`，或輸入 `$` 檢查能否選到 `$imagegen`。

若缺少 `$imagegen`：

1. 更新 Codex CLI。
2. 完全關閉舊 session，再啟動新 session。
3. 執行 `codex doctor`。
4. 檢查個人設定是否停用該 skill。
5. 檢查公司、學校或 ChatGPT workspace 是否禁止 skills 或圖片生成。
6. 若仍不可用，回報此環境不支援，或另外詢問使用者是否改走付費 API 路徑。

官方文件沒有提供適合長期硬編碼的 `$imagegen` 最低 CLI 版本。因此整合程式應做能力偵測，不應只比較版本號。

## CLI 呼叫方式

### 互動式

```text
$imagegen 生成一張中古奇幻酒館插圖，沒有文字。把最終 PNG 存到 ./generated/tavern.png。
```

### 非互動式

```bash
codex exec -C "/writable/output/root" \
  '$imagegen 生成一張中古奇幻酒館插圖，沒有文字。把最終 PNG 存到 ./generated/tavern.png，最後只回覆相對檔案路徑。'
```

若應用程式直接啟動 child process，應把包含 `$imagegen` 的整段 prompt 當成單一 argv 傳入，不要交給 shell 展開。

應用程式應指定可寫入的工作目錄與輸出檔名。不要假設 Codex 會把圖片寫在 stdout，也不要假設預設儲存位置。

## 圖片如何回傳

Codex CLI 的文字或 JSON 輸出適合回報圖片路徑，不適合直接承載二進位圖片。

建議整合契約：

```text
IMAGE_RESULT {"status":"ok","relative_path":"generated/tavern.png","mime":"image/png"}
```

呼叫端仍必須自行驗證：

- process 是否成功結束。
- 回傳路徑是否位於允許的輸出根目錄。
- 檔案是否存在且大小合理。
- MIME、magic bytes、副檔名是否一致。
- 圖片是否可成功解碼。
- 圖片尺寸是否在產品允許範圍。

不能只根據 exit code，也不能只根據 Codex 回覆「已完成」判定成功。

## 是否能改用圖片網路連結

不要要求 Codex 憑空產生一個圖片 URL。模型回覆的文字網址不代表圖片已被託管，也可能不存在、過期或無權限。

Codex 內建生圖的首選結果是本機檔案。

若產品一定需要 URL，正確流程是：

```text
Codex 生成本機圖片
  → 應用程式驗證圖片
  → 應用程式上傳到自己的物件儲存
  → 取得公開 URL 或 signed URL
```

「生圖」與「託管圖片」應視為兩個獨立能力。

若其他供應商的正式 API 本來就回傳暫存 URL，可由應用程式立即下載，但必須處理 URL 過期、SSRF、下載大小、重新導向、MIME 與圖片解碼驗證。

## 官方 Image API 備援

只有在 `$imagegen` 不可用、需要大量自動化，或產品需要直接控制 Image API 時，才考慮 API 備援。

API 備援需要另外準備：

1. OpenAI Platform API 帳號與有效的 `OPENAI_API_KEY`。
2. HTTPS client、官方 OpenAI SDK，或另外安裝官方 `openai` API CLI。
3. API 計費與 key 安全管理。
4. Base64 解碼與圖片檔案驗證。

這是另一套認證與計費路徑。不要因 `$imagegen` 暫時失敗就暗中切換到 API，也不要在未告知使用者時產生 API 費用。

Image API endpoint：

```text
POST https://api.openai.com/v1/images/generations
```

目前 GPT Image API 回傳：

```text
data[0].b64_json
```

呼叫端把 Base64 解碼後直接寫成 PNG、JPEG 或 WebP。這條路徑也不需要圖片 URL。

官方 API CLI 的生圖範例：

```bash
openai images generate \
  --model gpt-image-2 \
  --prompt "A cozy medieval fantasy tavern, no text" \
  --raw-output \
  --transform 'data.0.b64_json' |
  base64 --decode > tavern.png
```

注意：`openai` API CLI 與 `codex` CLI 是兩個不同程式。安裝 `codex` 不代表已安裝 `openai`。如果產品只採 Codex 內建生圖，就不需要安裝 `openai` CLI。

## Claude 的跨供應商比較資料

```yaml
provider: OpenAI
agent_cli:
  command: codex
  install:
    prerequisites:
      - Node.js
      - npm
    command: "npm install --global @openai/codex@latest"
    verify: "codex --version"
    login: "codex login"
    diagnose: "codex doctor"
image_generation:
  supported: true
  mechanism: "bundled system skill plus built-in image generation tool"
  invocation: "$imagegen"
  separate_imagegen_install_required: false
  current_builtin_image_model: "gpt-image-2"
  api_key_required: false
  billing: "general Codex usage limits"
  noninteractive: "codex exec"
  preferred_result: "local image file plus returned relative path"
  binary_in_stdout: false
  public_url_returned_by_default: false
capability_detection:
  - "start a new Codex CLI session"
  - "run /skills"
  - "confirm imagegen is available"
missing_capability_handling:
  - "update Codex"
  - "restart with a new session"
  - "run codex doctor"
  - "check local and workspace skill policy"
  - "do not install an untrusted similarly named package"
api_fallback:
  automatic: false
  requires_user_awareness: true
  requirements:
    - "OPENAI_API_KEY"
    - "API billing"
    - "HTTP client, official SDK, or separately installed openai API CLI"
    - "Base64 decoding"
  endpoint: "POST /v1/images/generations"
  result: "data[0].b64_json"
important_distinctions:
  - "codex CLI and openai API CLI are separate installations"
  - "image generation and image hosting are separate capabilities"
```

## Claude 整理方案時不可誤寫

- 不可寫成「使用者必須另外安裝 Image Gen plugin」。
- 不可把作者或研究者電腦上剛好存在的版本當成所有使用者的安裝前提。
- 不可把 `codex` 和 `openai` 當成同一支 CLI。
- 不可寫成內建 `$imagegen` 必須提供 `OPENAI_API_KEY`。
- 不可把模型文字回覆的 URL 當成真實圖片資產。
- 不可把 Base64 回傳描述成失敗；它是 Image API 的正常圖片傳輸方式。
- 不可在未告知使用者時，由 Codex 額度路徑切換到 Platform API 計費路徑。

## 官方來源

- [Codex Image generation](https://learn.chatgpt.com/docs/image-generation)
- [OpenAI Image generation API guide](https://developers.openai.com/api/docs/guides/image-generation)
- [Codex CLI commands](https://learn.chatgpt.com/docs/developer-commands)
- [Codex skills](https://learn.chatgpt.com/docs/build-skills)
- [OpenAI SDKs and CLI](https://developers.openai.com/api/docs/libraries)
- [OpenAI Codex repository](https://github.com/openai/codex)

