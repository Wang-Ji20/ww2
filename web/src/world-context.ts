import { createContext, useContext } from "react";
import type { World } from "./types";

/** 整局不变的静态世界，供各面板读取国家、兵种、地形与省份信息 */
export const WorldContext = createContext<World | null>(null);

export function useWorld(): World {
  const world = useContext(WorldContext);
  if (!world) throw new Error("useWorld 必须在 WorldContext 内使用");
  return world;
}
