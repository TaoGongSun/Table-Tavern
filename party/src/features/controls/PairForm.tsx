// 手動輸入配對碼（協定 §1.2）：QR 過期或掃錯時，出額度者照桌面版面板顯示的 XXXX-XXXX 自己輸入，成為出額度者（grantHost 碼另成為房主）。
import { useState } from "react";
import { t } from "../../i18n";
import { useRun, type Act } from "../room/useRoom";

export function PairForm({ act }: { act: Act }) {
  const { run, error } = useRun(act);
  const [code, setCode] = useState("");
  return (
    <form
      className="pair-form"
      onSubmit={async (e) => {
        e.preventDefault();
        if (await run("pair.claim", { code: code.trim() })) setCode("");
      }}
    >
      <label>
        {t("pairTitle")}
        <input
          value={code}
          maxLength={9}
          placeholder="XXXX-XXXX"
          autoCapitalize="characters"
          data-testid="pair-code"
          onChange={(e) => setCode(e.target.value)}
        />
      </label>
      <button type="submit" disabled={!code.trim()}>
        {t("pairButton")}
      </button>
      {error && (
        <div className="field-error" data-testid="pair-error">
          {error}
        </div>
      )}
    </form>
  );
}
