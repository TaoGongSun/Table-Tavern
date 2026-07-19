import { FormEvent, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [worlds, setWorlds] = useState<string[]>([]);
  const [worldName, setWorldName] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);

  async function refreshWorlds() {
    setWorlds(await invoke<string[]>("list_worlds"));
  }

  useEffect(() => {
    refreshWorlds()
      .catch((reason) => setError(String(reason)))
      .finally(() => setLoading(false));
  }, []);

  async function createWorld(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError("");
    try {
      await invoke("create_world", { name: worldName });
      setWorldName("");
      await refreshWorlds();
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <main className="container">
      <h1>桌面酒館</h1>

      <section aria-labelledby="world-list-title">
        <h2 id="world-list-title">世界</h2>
        {!loading && worlds.length === 0 ? (
          <p>尚無世界</p>
        ) : (
          <ul>
            {worlds.map((world) => (
              <li key={world}>{world}</li>
            ))}
          </ul>
        )}
      </section>

      <form className="row" onSubmit={createWorld}>
        <input
          aria-label="世界名稱"
          value={worldName}
          onChange={(event) => setWorldName(event.currentTarget.value)}
          placeholder="輸入世界名稱"
        />
        <button type="submit">建立世界</button>
      </form>
      {error && <p role="alert">{error}</p>}
    </main>
  );
}

export default App;
