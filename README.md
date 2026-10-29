# Table Tavern

**[繁體中文 →](README.zh-TW.md)**

AI plays a whole table of characters — and the GM (game master) too. Drop in one character card and just chat; want more depth, open a full table with a world setting, a lorebook, and scene changes, where the GM narrates and decides who speaks next. Each character only knows what the GM has said out loud in the story, so secrets really stay secret until they're revealed — no relying on the AI to "pretend not to know."

## Coming from SillyTavern? Bring your stuff

- **Character cards and lorebooks import directly** — PNG or JSON — and you can export your cards back out.
- **Cards with their own interface and variable system (MVU, including zod rules) still work**: the status bars, buttons, and stats the card defines show up and respond as written, and HTML layout and text styling in the story display like they do in SillyTavern.
- **Web saves carry over to the desktop app**, so you can keep playing there — conversation, variables, and lorebook state included.

## Play now

[![Play-in your browser](https://img.shields.io/badge/Play-in%20your%20browser-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![Download-Windows](https://img.shields.io/badge/Download-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/latest/download/TableTavern-Windows-Setup.exe)
[![Download-macOS](https://img.shields.io/badge/Download-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/latest/download/TableTavern-macOS.dmg)

**Web version**: open <https://table-tavern.pages.dev> and play — free, nothing to install. The web version runs one card per table; once you want more (multiple characters, a GM, your own worlds, your own subscription), download the desktop app.

Web saves live in your browser. Safari may clear data for sites you haven't opened in a while, so export saves you care about.

## Installing the desktop app

The download buttons above fetch the latest version directly. The release isn't code-signed yet, so your system will warn you the first time you open it. Do the steps below once — it's not malware.

**Windows**: if your browser says the file isn't commonly downloaded, choose **Keep** (in Edge it's under **…**, then **Keep anyway**). Run the installer you downloaded. If you see "Windows protected your PC": click **More info → Run anyway**.

**macOS** (Apple Silicon): open the `.dmg` you downloaded, drag `Table Tavern.app` into Applications, double-click. If macOS says it can't verify the app: press **Done**, open **System Settings → Privacy & Security**, scroll to the bottom, click **Open Anyway**, and confirm once.

When a new version comes out, the app lets you know and updates in one click; if you'd rather not keep it, you can roll back to the previous version from inside the app.

## Getting started

1. The first launch drops you into a sample table. Poke around.
2. Click **Connect OpenRouter** (OpenRouter is a service that connects you to many AI providers), sign in or sign up in your browser, approve — and you're back at the table. No key to copy and paste.
3. Start chatting. Don't feel like writing a card? Describe the story you want in one sentence and the app sets up a whole table.

## Why it's fun — and easy on your budget

- **Pick who speaks at multi-character tables**: **Let ○○ reply** appears under the GM's narration, and **Send + reply** sits next to the send button. Cards that provide setting but never speak (places, factions) can be made **stage cards**, so they're never asked to talk.
- **Play completely free**: once OpenRouter is connected, the app picks a good free model for you. If that one is busy or temporarily unavailable, it switches to the next one and tells you in the chat.
- **Use the subscription you already have**: with a Claude, ChatGPT, Gemini, or Grok subscription, you can route play through that provider's official command-line tool and spend the allowance you already pay for. The app installs it in one click, walks you through signing in, and shows **Connected** once it detects you're logged in. For Claude, Codex and Grok, the app prefers the tool version it was tested with (falling back to the latest if that can't be downloaded); Gemini always gets the latest. The app keeps the tool from updating itself while the app runs it; if a tool update breaks its output format, the app says so and offers **Check for CLI update**. Providers' rules on this kind of use differ; the app explains the risks before you switch, and the choice is yours.
- **Saves usage everywhere**:
  - Makes the most of AI providers' prompt caching, so the setup resent each turn isn't billed at full price again and again; the usage page shows your hit rate.
  - Card refactoring: for cards that reprint their whole interface every turn, the app takes over drawing the interface and the model only reports the values that changed — shorter replies, shorter waits.
  - Scene changes condense the story so far into a summary for the next scene, instead of hauling the whole conversation along.
  - The GM and each character can run on different model tiers — the strong one for who matters, a cheaper one for the extras.
  - Runaway output stops on its own, and you can stop any reply yourself.
- **AI character art**: generate a portrait for a character card. Needs a paid OpenRouter model or the ChatGPT / Gemini / Grok official command-line tool; the free tier can't generate images.
- **Your data stays on your computer**: tables, cards, and chat logs are local files, and conversations go straight from your computer to the AI service you picked — no server of ours in between.
- **Ten interface languages**: English, 繁體中文, 简体中文, 日本語, 한국어, Español, Português (Brasil), Deutsch, Français, Русский. The sample table follows your language too.

## FAQ

**Does it cost money?**
The app is free and open source, and you can play entirely on free models. For stronger paid models, top up your OpenRouter account and pay only for what you use, directly to the AI provider — no subscription.

**Can I sponsor?**
Yes. Sponsoring on Ko-fi gets you a sponsor pack; import it to unlock five extra color themes. Every feature works without sponsoring.

**Can characters peek at things they shouldn't know?**
No. Only the GM reads the world setting and everyone's cards; a character learns something only when the GM says it in the story. You can also limit lorebook entries to specific characters.

**Where is my data? How do I back it up?**
Everything is in `Documents/TableTavern/` — copy the folder and you've backed it all up. Settings and keys live separately in the system's app-config folder.

**Questions or suggestions?**
Drop them in [GitHub Issues](https://github.com/TaoGongSun/Table-Tavern/issues).

## For developers

The desktop app is Tauri 2 (Rust) + React/TypeScript; the web version is a separate Vite project in `web/`. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the architecture, [docs/STRUCTURE.md](docs/STRUCTURE.md) for where files go, and [CHANGELOG.md](CHANGELOG.md) for changes; releases are on [GitHub Releases](https://github.com/TaoGongSun/Table-Tavern/releases).

```bash
npm install
npm run tauri dev    # desktop dev mode
npm run tauri build  # installer / .app / DMG
npm run verify       # one-shot check: build, tests, i18n, structure
cd web && npm install && npm run dev   # web dev mode
```

## License

[AGPL-3.0-only](LICENSE).
