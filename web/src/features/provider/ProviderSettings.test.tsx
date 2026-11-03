// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeAll, describe, expect, it } from "vitest";
import { ProviderSettings } from "./ProviderSettings";
import type { CnProviderId } from "./providers";
import type { AiConnection } from "./useAiConnection";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

/** 魔搭的清單馬上回；矽基流動的清單一直不回（慢網路） */
function connection(provider: CnProviderId): AiConnection {
  const fetch = (provider === "modelscope"
    ? async () => new Response(JSON.stringify({ data: [{ id: "ms/only-model" }] }))
    : () => new Promise<Response>(() => {})) as typeof globalThis.fetch;
  return {
    provider,
    choices: ["modelscope", "siliconflow", "openrouter"],
    model: null,
    choose: () => {},
    chooseModel: () => {},
    keyed: { deps: { fetch, apiBase: `http://fake/${provider}` }, apiKey: "k" },
  } as unknown as AiConnection;
}

describe("model picker", () => {
  it("drops the previous provider's list at once when the provider changes, even while the new list is slow", async () => {
    const options = () => [...document.querySelectorAll("[data-testid=model-picker] option")].map((option) => option.getAttribute("value"));
    const host = document.body.appendChild(document.createElement("div"));
    const mount = createRoot(host);
    await act(async () => mount.render(<ProviderSettings connection={connection("modelscope")} busy={false} />));
    expect(options()).toEqual(["", "ms/only-model"]);
    await act(async () => mount.render(<ProviderSettings connection={connection("siliconflow")} busy={false} />));
    expect(options()).toEqual([""]);
    await act(async () => mount.unmount());
  });
});
