// 遊戲選單（Ren'Py game menu）：Esc、右鍵或快捷列「選單」開；桌名當標題，按鈕直列。重播桌不出生成類項目。
import { Panel } from "./StoryPanels";

export interface GameMenuItem {
  key: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
}

export function GameMenu({ title, items, onClose }: { title: string; items: GameMenuItem[]; onClose: () => void }) {
  return (
    <Panel label={title} onClose={onClose}>
      <div className="vn-story-list vn-game-menu">
        {items.map((item) => (
          <button key={item.key} type="button" className="btn" disabled={item.disabled} onClick={item.onSelect}>
            {item.label}
          </button>
        ))}
      </div>
    </Panel>
  );
}
