// 房間元件用的選單：沿用桌面版 MoreMenu 的鍵盤行為，彈層掛進自己所在的 .app（手機與嵌在桌面都一樣），
// 樣式在 party/src/App.css 的 `.app .menu` 底下覆寫，只用 party 的配色變數。
import { useLayoutEffect, useRef, useState, type ComponentProps } from "react";
import { MoreMenu } from "@desktop/shared/ui/MoreMenu";

export type PartyMenuItem = ComponentProps<typeof MoreMenu>["items"][number];

export function PartyMenu(props: Omit<ComponentProps<typeof MoreMenu>, "portalTo">) {
  const ref = useRef<HTMLSpanElement>(null);
  const [root, setRoot] = useState<Element | null>(null);
  useLayoutEffect(() => setRoot(ref.current?.closest(".app") ?? null), []);
  return (
    <span ref={ref} className="party-menu-anchor">
      <MoreMenu {...props} portalTo={root} />
    </span>
  );
}
