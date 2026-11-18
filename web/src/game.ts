import { useCallback, useEffect, useState } from "react";
import init, { Game } from "./wasm/ww2.js";
import { loadMods, type LoadedMods } from "./mods";
import { loadGeometry, type MapGeometry } from "./topology";
import type { GameState, Province, UserAction, World } from "./types";

export interface Setup {
  mod: string;
  scenario: string;
  /** 玩家国家；为空时使用剧本默认 */
  player: string | null;
}

export interface LoadedGame {
  game: Game;
  world: World;
  geometry: MapGeometry;
  mods: LoadedMods;
  setup: Setup;
}

let wasmReady: Promise<unknown> | null = null;

/** 加载 wasm 模块（只初始化一次） */
export function initEngine() {
  wasmReady ??= init();
  return wasmReady;
}

/** 读取模组、创建对局并解码地图轮廓；模组数据有误时抛出核心返回的全部错误 */
export async function loadGame(setup: Setup, mods?: LoadedMods): Promise<LoadedGame> {
  await initEngine();
  const loaded = mods ?? (await loadMods(setup.mod));
  const game = new Game(JSON.stringify(loaded.layers), setup.scenario, setup.player ?? undefined);
  const world = JSON.parse(game.worldJson()) as World;
  const provinces = Object.values(world.map.provinces) as Province[];
  const geometry = await loadGeometry(loaded.mapUrl, provinces);
  return { game, world, geometry, mods: loaded, setup };
}

function readState(game: Game): GameState {
  return JSON.parse(game.stateJson()) as GameState;
}

/** 以 requestAnimationFrame 驱动推演时钟，并在状态变化时重新读取快照 */
export function useGame(game: Game) {
  const [state, setState] = useState(() => readState(game));
  const refresh = useCallback(() => setState(readState(game)), [game]);

  useEffect(() => {
    if (state.is_paused || state.outcome !== null) return;
    let last = performance.now();
    let raf = requestAnimationFrame(function frame(now) {
      // 标签页切回时不追赶超过 1 秒的真实时间
      const dt = Math.min((now - last) / 1000, 1);
      last = now;
      if (game.tick(dt)) refresh();
      raf = requestAnimationFrame(frame);
    });
    return () => cancelAnimationFrame(raf);
  }, [game, refresh, state.is_paused, state.outcome]);

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
