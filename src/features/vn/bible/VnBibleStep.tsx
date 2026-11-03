// 精靈 ② 劇本（plans/vn-mode-pkg4.md §2.6）：一進來就生成聖經（帶 ① 選的劇透鎖），完成後用劇本分頁顯示
// （章數可改、其餘依鎖顯示遮罩或全文）；失敗時說明沒有聖經也能玩，可重試或直接下一步。
import { useEffect, useRef, useState } from "react";
import { t } from "../../../i18n";
import { backendText } from "../../../shared/ui/backend-text";
import { BibleTab } from "./BibleTab";
import { bibleErrorKey, useBible } from "./useBible";

export function VnBibleStep({ worldId, spoilerLock }: { worldId: string; spoilerLock: boolean }) {
  const { generate, view } = useBible(worldId);
  const [state, setState] = useState<"running" | "done" | "failed">("running");
  const [message, setMessage] = useState("");
  const started = useRef(false);

  async function run() {
    setState("running");
    try {
      await generate(spoilerLock);
      setState("done");
    } catch (reason) {
      const key = bibleErrorKey(reason);
      setMessage(key ? t(key) : backendText(reason));
      setState("failed");
    }
  }
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    // 已有聖經（從匯入或重開精靈回來）就不重生
    if (view?.bible) setState("done");
    else void run();
  }, []);

  if (state === "running") return <p className="vn-bible-progress">{t("vnBibleGenerating")}</p>;
  if (state === "failed") {
    return (
      <div className="vn-bible-failed">
        <p>{message}</p>
        <p>{t("vnBibleNone")}</p>
        <button type="button" className="btn" onClick={() => void run()}>
          {t("vnBibleRetry")}
        </button>
      </div>
    );
  }
  return <BibleTab worldId={worldId} />;
}
