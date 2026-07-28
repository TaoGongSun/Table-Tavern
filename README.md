# Table Tavern

**[繁體中文 →](README.zh-TW.md)**

A desktop app where AI plays a whole table of characters — and the Game Master too.

Play it as simple or as deep as you like: drop in a single character card and just chat, or run a full campaign with a world setting, a lorebook, scene changes, and a GM who narrates and decides who speaks next. Each character only knows what the GM says out loud, so secrets stay secret until the story reveals them — in a typical AI chat, secrets survive only as long as the AI keeps pretending not to know; here, the characters genuinely don't know.

Interface in English and Traditional Chinese; the built-in sample table follows your language.

## Download & Install

> Current builds are **test versions** — expect rough edges.

**macOS** (Apple Silicon): download the `.dmg`, drag `Table Tavern.app` into Applications, double-click.

If macOS says the app can't be verified: press **Done**, open **System Settings → Privacy & Security**, scroll to the bottom, click **Open Anyway**, confirm once. That's it — the warning appears because this test build isn't notarized with Apple yet.

**Windows**: download the installer from the release page. Windows may show an "unknown publisher" warning — click **More info → Run anyway**.

## Getting Started

1. First launch drops you straight into a sample table. Poke around freely.
2. To make characters talk, the app needs an AI service. The standard way: register at [OpenRouter](https://openrouter.ai/), top up a small amount, paste your key into the in-app guide. One key gives you many models, and the GM and each character can use different ones. Pay only for what you use — there is no subscription.
3. That's all. Write a card, join the table, play.

Already paying for Claude, ChatGPT, Gemini, or Grok? You can route conversations through their official command-line tools instead and spend your existing subscription — see the Q&A below.

## Q&A

**Is the app free?**
Yes — free and open source. The only cost is what your AI provider charges for usage, paid by you directly to them.

**Can I sponsor the project?**
We're still in the testing period — **please don't feel any need to sponsor yet.** The sponsor perks (five extra color themes and AI character-art generation) are finished, and sponsoring unlocks them right now; we just don't feel right asking for money while the experience is still being polished.

**Is my data uploaded anywhere?**
No. Tables, cards, and chat logs live on your computer as plain files, and your API key is stored only on your machine. Conversations go directly from you to the AI provider you chose — there is no middleman server.

**Can I use my SillyTavern character cards?**
Yes. Import V2 card PNGs directly, and export your cards back out as PNG or JSON.

**Can characters "peek" at things they shouldn't know?**
No. Only the GM reads the world setting and everyone's cards. A character learns something only when the GM actually says it in the story — you can even mark lorebook entries as visible to specific characters only.

**Can I play on my existing Claude / ChatGPT / Gemini / Grok subscription?**
Yes — in AI settings, switch the transport to that provider's official CLI (one-click install included). Provider policies on this kind of use differ and change; the app shows each provider's risk notes before you enable it, so you can decide for yourself.

**Where is my data? How do I back it up?**
Everything is in `Documents/TableTavern/` — copy that folder and you've backed up every table, card, and log. Settings and keys are in the system app-config folder, separate from your stories.

**The app won't open / got blocked.**
See the install section above — it's the standard warning for unsigned test builds, not a malware verdict.

## For Developers

Tauri 2 (Rust) + Vite + React + TypeScript. Product spec in `NewPlan.md`, engineering kickoff in `KICKOFF.md`, changes in [CHANGELOG.md](CHANGELOG.md).

```bash
npm install
npm run tauri dev    # dev mode
npm run tauri build  # .app / DMG / installer
cd src-tauri && cargo test
npm run build        # front-end type check + build
```

## License

[AGPL-3.0-only](LICENSE).
