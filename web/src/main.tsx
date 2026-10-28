import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { loadGame } from "./game";
import "./styles.css";

const root = createRoot(document.getElementById("root")!);

loadGame()
  .then(({ game, catalog }) =>
    root.render(
      <StrictMode>
        <App game={game} catalog={catalog} />
      </StrictMode>,
    ),
  )
  .catch((err) => root.render(<p className="fatal">无法加载推演引擎：{String(err)}</p>));
