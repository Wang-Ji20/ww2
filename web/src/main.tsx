import { lazy, Suspense, StrictMode, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
const App = lazy(() => import("./App").then((module) => ({ default: module.App })));
import { StartMenu } from "./components/StartMenu";
import type { LoadedGame, Setup } from "./game";
import type { LoadedMods } from "./mods";
import "./styles.css";

type Screen = { kind: "menu"; error: string | null } | { kind: "loading" } | { kind: "game"; loaded: LoadedGame };

function Root() {
  const [screen, setScreen] = useState<Screen>({ kind: "menu", error: null });
  const [lastSetup, setLastSetup] = useState<Setup | null>(null);
  // 上一局的 wasm 对象：此时其界面已卸载、时钟已停止，可以安全释放
  const previous = useRef<LoadedGame | null>(null);

  const start = (setup: Setup, mods: LoadedMods) => {
    previous.current?.game.free();
    previous.current = null;
    setLastSetup(setup);
    setScreen({ kind: "loading" });
    import("./game").then(({ loadGame }) => loadGame(setup, mods))
      .then((loaded) => {
        previous.current = loaded;
        setScreen({ kind: "game", loaded });
      })
      // 模组数据错误由核心逐条列出
      .catch((err) => setScreen({ kind: "menu", error: `无法开局：\n${err instanceof Error ? err.message : String(err)}` }));
  };

  switch (screen.kind) {
    case "menu":
      return <StartMenu initial={lastSetup} error={screen.error} onStart={start} />;
    case "loading":
      return <p className="loading">正在构建世界…</p>;
    case "game":
      return <Suspense fallback={<p className="loading" role="status">正在展开战区地图…</p>}><App loaded={screen.loaded} onMenu={() => setScreen({ kind: "menu", error: null })} /></Suspense>;
  }
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
