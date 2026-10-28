import { useCallback, useEffect, useState } from "react";
import init, { Game, catalogJson } from "./wasm/ww2.js";
import type { Catalog, GameState, UserAction } from "./types";

/** 加载 wasm 模块并创建 1939 开局 */
export async function loadGame(): Promise<{ game: Game; catalog: Catalog }> {
  await init();
  return { game: new Game(), catalog: JSON.parse(catalogJson()) as Catalog };
}

function readState(game: Game): GameState {
  return JSON.parse(game.stateJson()) as GameState;
}

/** 以 requestAnimationFrame 驱动推演时钟，并在状态变化时重新读取快照 */
export function useGame(game: Game) {
  const [state, setState] = useState(() => readState(game));
  const refresh = useCallback(() => setState(readState(game)), [game]);

  useEffect(() => {
    let last = performance.now();
    let raf = requestAnimationFrame(function frame(now) {
      // 标签页切回时不追赶超过 1 秒的真实时间
      const dt = Math.min((now - last) / 1000, 1);
      last = now;
      if (game.tick(dt)) refresh();
      raf = requestAnimationFrame(frame);
    });
    return () => cancelAnimationFrame(raf);
  }, [game, refresh]);

  const act = useCallback(
    (action: UserAction) => {
      game.act(JSON.stringify(action));
      refresh();
    },
    [game, refresh],
  );

  const restart = useCallback(() => {
    game.reset();
    refresh();
  }, [game, refresh]);

  return { state, act, restart };
}
