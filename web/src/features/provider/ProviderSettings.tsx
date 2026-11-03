// 展開路線：換供應商、中國供應商可手選模型（含付費，要標明）。回合進行中整組停用：
// 舊回合收尾前不換 session，晚到的結果就不會寫進新供應商的桌（方案「切換供應商或模型時的回合」）。
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { fetchModelIds } from "./fixed-pool";
import { isCn, priceTag, type CnProviderId, type ProviderId } from "./providers";
import type { AiConnection } from "./useAiConnection";

export const providerName = (id: ProviderId) => (id === "openrouter" ? "OpenRouter" : t(`provider_${id}`));

const TAG_TEXT = { free: "modelFree", paid: "modelPaid", "maybe-paid": "modelMaybePaid" } as const;

function ModelPicker({ connection, provider, disabled }: { connection: AiConnection; provider: CnProviderId; disabled: boolean }) {
  const { deps, apiKey } = connection.keyed;
  const [ids, setIds] = useState<string[]>([]);
  useEffect(() => {
    let live = true;
    void fetchModelIds(deps, apiKey).then((result) => live && setIds(Array.isArray(result) ? result : []));
    return () => {
      live = false;
    };
  }, [deps, apiKey]);
  const chosen = connection.model;
  // 清單抓不到時，已選的那支照樣列著
  const options = chosen !== null && !ids.includes(chosen) ? [chosen, ...ids] : ids;
  const label = (id: string) => {
    const tag = priceTag(provider, id);
    return tag ? `${id} · ${t(TAG_TEXT[tag])}` : id;
  };
  const chosenTag = chosen === null ? null : priceTag(provider, chosen);
  return (
    <label className="settings-row">
      {t("settingsModel")}
      <select
        data-testid="model-picker"
        value={chosen ?? ""}
        disabled={disabled}
        onChange={(event) => connection.chooseModel(event.target.value || null)}
      >
        <option value="">{t("settingsModelAuto")}</option>
        {options.map((id) => (
          <option key={id} value={id}>
            {label(id)}
          </option>
        ))}
      </select>
      {(chosenTag === "paid" || chosenTag === "maybe-paid") && (
        <span className="settings-paid" role="note">
          {t("modelPaidNote", { provider: providerName(provider) })}
        </span>
      )}
    </label>
  );
}

/** 展開入口：打開才掛設定（模型清單打開才抓，不在每次開站多打一個請求） */
export function SettingsMenu({
  className,
  summary,
  testId,
  connection,
  busy,
}: {
  className: string;
  summary: string;
  testId?: string;
  connection: AiConnection;
  busy: boolean;
}) {
  const [open, setOpen] = useState(false);
  return (
    <details className={className} onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary data-testid={testId}>{summary}</summary>
      {open && <ProviderSettings connection={connection} busy={busy} />}
    </details>
  );
}

export function ProviderSettings({ connection, busy }: { connection: AiConnection; busy: boolean }) {
  const current = connection.provider;
  return (
    <fieldset className="provider-settings" disabled={busy} data-testid="provider-settings">
      <legend>{t("settingsProvider")}</legend>
      {connection.choices.map((id) => (
        <label key={id} className="settings-choice">
          <input type="radio" name="provider" checked={current === id} onChange={() => connection.choose(id)} />
          {providerName(id)}
        </label>
      ))}
      {/* 依供應商重掛：上一家的清單不留到新清單抓回來之前 */}
      {isCn(current) && <ModelPicker key={current} connection={connection} provider={current} disabled={busy} />}
      {busy && <p className="settings-busy">{t("settingsBusy")}</p>}
    </fieldset>
  );
}
