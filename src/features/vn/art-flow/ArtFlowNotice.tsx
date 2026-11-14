import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { t } from "../../../i18n";
import { CLI_LABELS } from "../../ai-connection/cli";
import { focusFrom } from "../../../shared/ui/MoreMenu";
import { updateConfig } from "../../settings/update-config";
import { flushArtStyle } from "./art-style-save";
import { ArtStyleFields } from "./ArtStyleFields";
import { useArtStatus } from "./useArtStatus";
import { useArtStyle } from "./useArtStyle";
import { useQuotaRecovery } from "./useQuotaRecovery";
import { SpriteSourceSelect } from "./SpriteSourceSelect";

/** 美術進度與畫風（美術與音樂面板頂端、精靈 ③）；compact＝標題畫面的一行摘要，被拒絕時帶一顆去面板處理的按鈕 */
export function ArtFlowNotice({ worldId, onStarted, onPause, onOpenGallery, compact = false }: { worldId: string; onStarted?: (started: boolean) => void; onPause?: (paused: boolean) => void; onOpenGallery?: () => void; compact?: boolean }) {
  const { status, read } = useArtStatus(worldId);
  const [source, setSource] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [batch, setBatch] = useState<number | null>(null);
  // 第一次讀到狀態才當輸入框起點（之後以輸入框為準）
  const style = useArtStyle(worldId, status && !compact ? { style: status.art.style ?? "", avoid: status.art.avoid ?? "" } : null);
  const styleInput = useRef<HTMLTextAreaElement>(null);
  const sourceBox = useRef<HTMLDivElement>(null);
  /** 生圖動作：先等畫風存好（存不起來就不送），再呼叫後端 */
  const act = (run: () => Promise<unknown>) => {
    setBusy(true); setError(false);
    void flushArtStyle(worldId).then(run).then(read).catch(() => setError(true)).finally(() => setBusy(false));
  };
  const retry = (withoutStyle = false) => act(() => invoke("vn_art_retry", { worldId, withoutStyle }));
  useEffect(() => { if (status) { onStarted?.(status.art.started); onPause?.(Boolean(status.pause)); } }, [status, onStarted, onPause]);
  const remaining = status ? status.total - status.ready + status.backgrounds : 0;
  const refused = status?.refused ?? 0;
  const recovery = useQuotaRecovery(status?.pause?.reset ?? null);
  const start = <button type="button" className="btn btn-primary" disabled={!status || !source || busy} onClick={() => {
    setBatch(remaining);
    act(() => updateConfig({ preferences: { vn_sprite_source: source } }).then(() => invoke("vn_art_start", { worldId })));
  }}>{t("vnArtBegin")}</button>;
  const retryButton = <button type="button" className="btn btn-sm" disabled={busy} onClick={() => retry()}>{t("vnArtRetryNow")}</button>;
  if (compact && (!status || (!status.pause && !status.failed && !refused && status.ready >= status.total))) return null;
  if (compact) return <div className="vn-art-summary">
    <span>{status!.pause ? t("vnArtQuotaShort", { time: recovery }) : refused ? t("vnArtRefused", { n: refused }) : status!.failed ? t("vnArtFailed", { n: status!.failed }) : status!.art.started ? t("vnArtPreparing", { n: status!.ready, total: status!.total }) : t("vnArtNoSprites", { panel: t("vnGallery") })}</span>
    {!status!.pause && refused > 0 && onOpenGallery
      ? <button type="button" className="btn btn-sm" onClick={onOpenGallery}>{t("vnArtOpenGallery")}</button>
      : (status?.pause || Boolean(status?.failed) || refused > 0) && retryButton}
    {error && <span className="field-warn">{t("vnArtActionFailed")}</span>}
  </div>;
  const expressions = status?.facesPerCharacter != null
    ? t("vnArtExpressionsEach", { each: status.facesPerCharacter })
    : t("vnArtExpressionsTotal", { n: status?.faces ?? 0 });
  const hasStyle = Boolean(style.value?.style.trim());
  // 已開始、沒暫停、佇列裡只剩失敗或被拒的：沒有圖在畫，不顯示「準備中」
  const stuck = (status?.failed ?? 0) + refused;
  const stalled = Boolean(status?.art.started && !status.pause && stuck > 0 && status.pending <= stuck);
  // 按鈕是滑鼠點的，引擎不會替 select 畫焦點框；明講要外框，玩家才看得出焦點跳到來源選單
  const focusSource = () => focusFrom(sourceBox.current?.querySelector("select"), "keyboard");
  return <div className="vn-art-flow">
    <div className="vn-art-controls">
      <div ref={sourceBox}><SpriteSourceSelect onChange={setSource} onSelect={status?.art.started ? retry : undefined} disabled={busy} /></div>
      {!status?.art.started && start}
      {status?.art.started && (status.pause || Boolean(status.failed)) && retryButton}
    </div>
    {style.value && <ArtStyleFields value={style.value} onChange={style.edit} onBlur={() => void style.save()} disabled={busy} styleRef={styleInput}>
      {style.state === "failed" ? <p className="field-warn">{t("vnStyleSaveFailed")}</p>
        : style.changed && status?.art.started ? <p className="field-note">{t("vnStyleChanged")}</p>
        : style.state === "saved" && <p className="field-note">{t("vnStyleSaved")}</p>}
    </ArtStyleFields>}
    {refused > 0 && <div className="vn-art-refused">
      <p className="field-warn">{t("vnArtRefused", { n: refused })}</p>
      <div className="vn-art-refused-actions">
        {hasStyle && <button type="button" className="btn btn-sm" onClick={() => styleInput.current?.focus()}>{t("vnArtEditStyle")}</button>}
        {hasStyle && <button type="button" className="btn btn-sm" disabled={busy} onClick={() => retry(true)}>{t("vnArtRetryPlain")}</button>}
        <button type="button" className="btn btn-sm" onClick={focusSource}>{t("vnArtChangeSource")}</button>
        {!hasStyle && <button type="button" className="btn btn-sm" disabled={busy} onClick={() => retry()}>{t("vnArtRetryNow")}</button>}
      </div>
    </div>}
    {!stalled && <p>{status?.pause ? t("vnArtStopped", { n: batch === null ? status.ready : Math.max(0, batch - remaining), total: batch ?? status.total + status.backgrounds }) : status?.art.started ? remaining === 0 && status.pending === 0 ? t("vnArtReady") : batch === null ? t("vnArtPreparing", { n: status.ready, total: status.total }) : t("vnArtWorking", { n: Math.max(0, batch - remaining), total: batch }) : t("vnArtNotStarted")}</p>}
    {source && !status?.pause && <p className="field-note">{t("vnArtEstimate", { source: CLI_LABELS[source] ?? "API", n: status ? status.total - status.ready : 0, bg: status?.backgrounds ?? 0 })}</p>}
    {source && !status?.pause && <p className="field-note">{expressions}</p>}
    {Boolean(status?.failed) && <p className="field-warn">{t("vnArtFailed", { n: status?.failed ?? 0 })}</p>}
    {status?.pause && <p className="field-warn">{status.pause.reset !== null ? t("vnArtPaused", { n: remaining, time: recovery }) : t("vnArtPausedUnknown", { n: remaining })}</p>}
    {error && <p className="field-warn">{t("vnArtActionFailed")}</p>}
  </div>;
}
