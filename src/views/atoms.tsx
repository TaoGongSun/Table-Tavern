import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../i18n";
import { renderStoryMarkdown } from "../shared/ui/story-markdown";
import { explainAiError } from "../shared/ui/ai-error";
import { IconBack, IconExport } from "../shared/ui/icons";
import { TranscriptEvent } from "../shared/contracts/backend-contracts";

// 錯誤列：命中分流就顯示人話，原始字串一律保留在小字（玩家與協助者仍看得到真相）。
// transport 給得出來就傳：認證失敗要指對地方（API 換金鑰／CLI 重新登入）。
export function ErrorNote({ text, transport }: { text: string; transport?: string }) {
  const key = explainAiError(text, transport);
  if (!key) return <p role="alert">{text}</p>;
  return (
    <p role="alert">
      {t(key)}
      <br />
      <small>{text}</small>
    </p>
  );
}


export function StoryText({ text }: { text: string }) {
  const html = useMemo(() => renderStoryMarkdown(text), [text]);
  return (
    <span
      className="text rendered"
      // 卡片內嵌的圖是熱連作者自己的圖床，失效是常態（擋熱連、圖被刪、玩家離線）：
      // 載不到就整張藏掉，故事中間留一個破圖示比沒有更糟。img 的 error 不冒泡，只能用 capture 收
      onErrorCapture={(event) => {
        const target = event.target as HTMLElement;
        if (target.tagName === "IMG") target.classList.add("img-failed");
      }}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

// 單幕閱讀：整面取代對話畫面（不是 modal），頂列外觀同編輯頁（返回、標題、匯出、續玩），
// 下方唯讀事件列表填滿到底
export function ActReader({
  world,
  worldName,
  scene,
  label,
  onBack,
  onFork,
}: {
  world: string;
  worldName: string;
  scene: number;
  label: string;
  onBack: () => void;
  onFork: () => void;
}) {
  const [events, setEvents] = useState<TranscriptEvent[] | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    setEvents(null);
    setError("");
    invoke<TranscriptEvent[]>("read_transcript", { worldId: world, scene })
      .then(setEvents)
      .catch((reason) => setError(String(reason)));
  }, [world, scene]);

  async function exportScene() {
    setError("");
    try {
      const now = new Date();
      const pad = (n: number) => String(n).padStart(2, "0");
      const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}${pad(now.getMinutes())}`;
      const path = await saveDialog({
        defaultPath: `${t("sceneExportFileName", { table: worldName, n: scene + 1, stamp })}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await invoke("export_scene", { worldId: world, scene, path });
      await revealItemInDir(path);
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <>
      <header className="edit-page-bar">
        <button type="button" className="btn btn-ghost edit-page-back" onClick={onBack}>
          <IconBack />
          {t("backToNow")}
        </button>
        <h2 className="edit-page-title" title={label}>
          {label}
        </h2>
        <span className="toolbar-spacer" />
        <button
          type="button"
          className="btn btn-shrink"
          title={t("exportScene")}
          onClick={exportScene}
        >
          <IconExport />
          <span className="btn-label">{t("exportScene")}</span>
        </button>
        {/* 分岔續玩：整面畫面唯一往前推進的動作，是這頁的主鈕 */}
        <button
          type="button"
          className="btn btn-primary btn-shrink"
          title={t("sceneFork")}
          onClick={onFork}
        >
          <span className="btn-label">{t("sceneFork")}</span>
        </button>
      </header>
      <section className="messages" aria-label={label}>
        {events === null ? (
          error && <ErrorNote text={error} />
        ) : (
          events.map((event, index) => (
            <div key={index} className={`scene-event scene-event-${event.kind}`}>
              {(event.kind === "dialogue" || event.kind === "player") && (
                <span className="speaker">{event.speaker_name}</span>
              )}
              <StoryText text={event.text} />
            </div>
          ))
        )}
      </section>
      {error && events !== null && <ErrorNote text={error} />}
    </>
  );
}
