import { useState } from "react";
import { Dialog } from "../../shared/ui/Dialog";
import { diceCopy } from "./dice-ui";

const SOURCES: [string, string, boolean?][] = [
  ["D&D SRD 5.1", "https://www.dndbeyond.com/srd"],
  ["D&D SRD 5.2", "https://www.dndbeyond.com/srd"],
  [
    "Pathfinder 2e Remaster",
    "https://paizo.com/blog/pathfinder-second-edition-remaster-project",
  ],
  ["Fate", "https://fate-srd.com/official-licensing-fate", true],
  ["Cypher CSRD", "https://csol.montecookgames.com/license/"],
  [
    "Year Zero Engine SRD",
    "https://freeleaguepublishing.com/community-content/free-tabletop-licenses/",
    true,
  ],
  ["Dungeon World", "https://www.dungeon-world.com/about/"],
  ["Basic Fantasy 4e", "https://www.basicfantasy.org/downloads.html"],
  ["Cairn", "https://cairnrpg.com/"],
  [
    "BRP",
    "https://www.chaosium.com/content/orclicense/BasicRoleplaying-ORC-Content-Document.pdf",
  ],
  ["Cthulhu Eternal", "https://cthulhueternal.com/open-rulesets/"],
  ["Blades in the Dark SRD", "https://bladesinthedark.com/licensing", true],
  ["Ironsworn SRD", "https://tomkinpress.com/pages/licensing", true],
];
export function RulesGuide({ compact = false, room = false }: { compact?: boolean; room?: boolean }) {
  const [open, setOpen] = useState(false);
  const c = diceCopy();
  return (
    <>
      <button
        type="button"
        className="btn btn-sm"
        title={c.guide}
        aria-label={c.guide}
        onClick={() => setOpen(true)}
      >
        {compact ? "?" : c.guide}
      </button>
      {open && (
        <Dialog title={c.guide} className="dice-guide" closeButton onDismiss={() => setOpen(false)}>
          <p>{c.intro}</p>
          <p>{c.sources}</p>
          <ul>
            {SOURCES.map(([name, url, unsupported]) => (
              <li key={name}>
                <a href={url} target="_blank" rel="noreferrer">
                  {name}
                </a>
                {unsupported && ` · ${c.unsupported}`}
              </li>
            ))}
          </ul>
          <p>{c.notice}</p>
          <p>{room ? c.room : c.soloGuide}</p>
        </Dialog>
      )}
    </>
  );
}
