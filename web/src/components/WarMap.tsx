import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { areAllies, countryColor, countryName } from "../display";
import { copyOffsets, geometryLevel, hitTest, isTiny, linkKey, TINY_PX, type MapGeometry, type Point } from "../topology";
import type { Combat, CountryId, GameState, ProvinceId, Targeting, World } from "../types";

interface Props {
  state: GameState;
  world: World;
  geometry: MapGeometry;
  targeting: Targeting;
  onClick: (id: ProvinceId) => void;
  onQuickOrder: (id: ProvinceId, allUnits: boolean) => void;
}

interface View {
  scale: number;
  tx: number;
  ty: number;
}

interface March {
  from: ProvinceId;
  to: ProvinceId;
  country: CountryId;
  progress: number;
  count: number;
}

/** 超过该缩放时绘制省内边界与地名 */
const DETAIL_SCALE = 1.6;
/** 拖动超过该像素视为平移而非点击 */
const DRAG_PX = 4;

/** 底图离屏画布在视口四周多绘制的像素 */
const BASE_MARGIN = 256;
/** 缩放过程中底图最多每隔这么久按新比例重绘一次（毫秒） */
const ZOOM_REFRESH_MS = 100;

type Bbox = [number, number, number, number];

/** 读取地图主题色 */
function readColors() {
  const style = getComputedStyle(document.documentElement);
  const v = (name: string) => style.getPropertyValue(name).trim();
  return {
    sea: v("--sea"),
    seaLane: v("--sea-lane"),
    provinceLine: v("--province-line"),
    border: v("--border-line"),
    front: v("--front-line"),
    accent: v("--accent"),
    danger: v("--danger"),
  };
}

export function WarMap({ state, world, geometry, targeting, onClick, onQuickOrder }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [view, setView] = useState<View | null>(null);
  const [hover, setHover] = useState<{ id: ProvinceId; x: number; y: number } | null>(null);
  const hoverId = hover?.id ?? null;
  const colors = useMemo(readColors, []);
  const drag = useRef<{ x: number; y: number; view: View; moved: boolean } | null>(null);
  const hitCtx = useMemo(() => document.createElement("canvas").getContext("2d")!, []);

  /* ── 尺寸与初始视角 ── */

  useLayoutEffect(() => {
    const el = wrapRef.current!;
    const ro = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setSize({ w: Math.floor(width), h: Math.floor(height) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const fitScale = useMemo(() => {
    const [x0, y0, x1, y1] = geometry.bounds;
    return size.w && size.h ? Math.min(size.w / (x1 - x0), size.h / (y1 - y0)) : 1;
  }, [geometry, size]);

  // 首次布局：以玩家首都为中心，真实地图放大到约 60 个经度宽
  useEffect(() => {
    if (view || !size.w) return;
    const [cx, cy] = geometry.shapes.get(world.capitals[state.player_country])!.center;
    const scale = geometry.hasShapes ? Math.max(fitScale, size.w / 600) : fitScale * 0.9;
    const [x0, y0, x1, y1] = geometry.bounds;
    const center = geometry.hasShapes ? [cx, cy] : [(x0 + x1) / 2, (y0 + y1) / 2];
    setView({ scale, tx: size.w / 2 - center[0] * scale, ty: size.h / 2 - center[1] * scale });
  }, [view, size, fitScale, geometry, world, state.player_country]);

  // 环绕地图上把视野中心的经度保持在 [-180°, 180°)，使平移可以无限进行
  const normalize = useCallback(
    (v: View): View => {
      const period = geometry.wrap;
      if (!period) return v;
      const centerX = (size.w / 2 - v.tx) / v.scale;
      const k = Math.floor((centerX + period / 2) / period);
      return k ? { ...v, tx: v.tx + k * period * v.scale } : v;
    },
    [geometry, size.w],
  );
  const commit = useCallback((v: View) => setView(normalize(v)), [normalize]);

  // 最小缩放：环绕地图不显示超过一周的宽度
  const minScale = geometry.wrap ? Math.max(fitScale * 0.8, size.w / geometry.wrap) : fitScale * 0.8;

  const toWorld = useCallback(
    (sx: number, sy: number): Point => (view ? [(sx - view.tx) / view.scale, (sy - view.ty) / view.scale] : [0, 0]),
    [view],
  );

  // 键盘切换选中省份时，若其离开视野则平移过去
  useEffect(() => {
    if (!view || !size.w) return;
    const [cx, y] = geometry.shapes.get(state.selected_province)!.center;
    // 环绕地图上取离视野中心最近的副本
    const viewX = (size.w / 2 - view.tx) / view.scale;
    const x = geometry.wrap ? cx + Math.round((viewX - cx) / geometry.wrap) * geometry.wrap : cx;
    const sx = x * view.scale + view.tx;
    const sy = y * view.scale + view.ty;
    const margin = 60;
    if (sx < margin || sy < margin || sx > size.w - margin || sy > size.h - margin) {
      commit({ ...view, tx: size.w / 2 - x * view.scale, ty: size.h / 2 - y * view.scale });
    }
    // 只响应选中省份的变化
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.selected_province]);

  /* ── 由动态状态派生的绘制数据 ── */

  // 控制权与交战关系决定底图（填色、国界、前线）；其余状态只影响叠加层
  const controllersKey = useMemo(() => Object.values(state.controllers).join(","), [state.controllers]);
  const enemiesKey = useMemo(
    () => Object.values(state.countries).map((c) => `${c.id}:${c.enemies.join("|")}`).join(","),
    [state.countries],
  );

  // 非陆地接壤的邻接：海峡与远洋航线（节点图中为全部邻接）
  const seaLinks = useMemo(() => {
    const links: [ProvinceId, ProvinceId][] = [];
    for (const p of Object.values(world.map.provinces)) {
      for (const n of p.neighbors) {
        if (p.id < n && !geometry.landLinks.has(linkKey(p.id, n))) links.push([p.id, n]);
      }
    }
    return links;
  }, [world, geometry]);

  const capitals = useMemo(() => new Set(Object.values(world.capitals)), [world]);

  const { unitCounts, marches } = useMemo(() => {
    const unitCounts = new Map<ProvinceId, Map<CountryId, number>>();
    const marches = new Map<string, March>();
    for (const d of Object.values(state.divisions)) {
      const counts = unitCounts.get(d.location) ?? new Map<CountryId, number>();
      counts.set(d.country, (counts.get(d.country) ?? 0) + 1);
      unitCounts.set(d.location, counts);

      if (d.order.kind === "Moving" && d.order.path.length > 0) {
        const to = d.order.path[0];
        const key = `${d.location}-${to}-${d.country}`;
        const progress = Math.min(d.order.progress_km / d.order.distance_km, 1);
        const m = marches.get(key);
        if (m) {
          m.progress = Math.max(m.progress, progress);
          m.count += 1;
        } else {
          marches.set(key, { from: d.location, to, country: d.country, progress, count: 1 });
        }
      }
    }
    return { unitCounts, marches: [...marches.values()] };
  }, [state.divisions]);

  const combatAt = useMemo(
    () => new Map<ProvinceId, Combat>(Object.values(state.combats).map((c) => [c.province, c])),
    [state.combats],
  );

  const hatchCache = useRef(new Map<string, CanvasPattern>());
  const hatch = useCallback(
    (ctx: CanvasRenderingContext2D, color: string) => {
      let pattern = hatchCache.current.get(color);
      if (!pattern) {
        const tile = document.createElement("canvas");
        tile.width = tile.height = 8;
        const t = tile.getContext("2d")!;
        t.strokeStyle = color;
        t.lineWidth = 2.5;
        t.beginPath();
        t.moveTo(-2, 10);
        t.lineTo(10, -2);
        t.stroke();
        pattern = ctx.createPattern(tile, "repeat")!;
        hatchCache.current.set(color, pattern);
      }
      return pattern;
    },
    [],
  );

  /* ── 绘制 ── */

  // 底图缓存在离屏画布上，四周多留 BASE_MARGIN；平移在余量内、缩放过程中只做位图变换
  const base = useRef<{ canvas: HTMLCanvasElement; view: View; key: string; at: number } | null>(null);
  useEffect(() => {
    base.current = null;
  }, [world, geometry]);
  const paint = useRef(() => {});
  const frame = useRef(0);
  const refreshTimer = useRef(0);
  const requestPaint = useCallback(() => {
    frame.current ||= requestAnimationFrame(() => {
      frame.current = 0;
      paint.current();
    });
  }, []);
  useEffect(
    () => () => {
      cancelAnimationFrame(frame.current);
      frame.current = 0;
      clearTimeout(refreshTimer.current);
    },
    [],
  );

  useEffect(() => {
    paint.current = () => {
      const canvas = canvasRef.current;
      if (!canvas || !view || !size.w) return;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const pixelWidth = Math.round(size.w * dpr);
      const pixelHeight = Math.round(size.h * dpr);
      if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
        canvas.width = pixelWidth;
        canvas.height = pixelHeight;
      }
      const ctx = canvas.getContext("2d")!;
      const { scale, tx, ty } = view;
      const level = geometryLevel(geometry, scale);
      const period = geometry.wrap;
      const toScreen = ([x, y]: Point, offset = 0): Point => [(x + offset) * scale + tx, y * scale + ty];
      const centerOf = (id: ProvinceId) => geometry.shapes.get(id)!.center;
      const detail = scale >= DETAIL_SCALE;
      const [vx0, vy0] = [-tx / scale, -ty / scale];
      const [vx1, vy1] = [(size.w - tx) / scale, (size.h - ty) / scale];
      // 与视野相交的各个环绕副本的横向偏移
      const visibleCopies = (b: readonly number[]) => (b[3] >= vy0 && b[1] <= vy1 ? copyOffsets(b, vx0, vx1, period) : []);
      // 从 a 到 b 走较短的一侧（跨越 180° 经线时 b 换成相邻副本）
      const nearest = (a: Point, b: Point): Point => (period ? [b[0] + Math.round((a[0] - b[0]) / period) * period, b[1]] : b);
      const segmentBox = (a: Point, b: Point): Bbox => [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
      const shift = (offset: number) => new DOMMatrix([1, 0, 0, 1, offset, 0]);
      // 把路径复制到各个可见副本
      const copies = (path: Path2D, bbox: readonly number[]) => {
        const out = new Path2D();
        for (const o of visibleCopies(bbox)) out.addPath(path, shift(o));
        return out;
      };

      // 底图：尺寸、控制权或交战关系变化，平移超出余量，或缩放后间隔足够时重绘
      const key = `${size.w}x${size.h}@${dpr}|${geometry.bounds}|${controllersKey}|${enemiesKey}`;
      const now = performance.now();
      const current = view;
      const stale = (cached: NonNullable<typeof base.current>) => {
        if (cached.key !== key) return true;
        const k = scale / cached.view.scale;
        const left = tx - cached.view.tx * k - BASE_MARGIN * k;
        const top = ty - cached.view.ty * k - BASE_MARGIN * k;
        const covers =
          left <= 0 && top <= 0 && left + (size.w + 2 * BASE_MARGIN) * k >= size.w && top + (size.h + 2 * BASE_MARGIN) * k >= size.h;
        return !covers || (k !== 1 && now - cached.at >= ZOOM_REFRESH_MS);
      };
      const cached = base.current;
      const b = cached && !stale(cached) ? cached : renderBase(cached?.canvas ?? document.createElement("canvas"));
      base.current = b;

      function renderBase(out: HTMLCanvasElement) {
        const bw = size.w + 2 * BASE_MARGIN;
        const bh = size.h + 2 * BASE_MARGIN;
        if (out.width !== Math.round(bw * dpr) || out.height !== Math.round(bh * dpr)) {
          out.width = Math.round(bw * dpr);
          out.height = Math.round(bh * dpr);
        }
        const g = out.getContext("2d")!;
        const [ox, oy] = [tx + BASE_MARGIN, ty + BASE_MARGIN];
        const [bx0, by0] = [-ox / scale, -oy / scale];
        const [bx1, by1] = [(bw - ox) / scale, (bh - oy) / scale];
        const baseCopies = (b: readonly number[]) => (b[3] >= by0 && b[1] <= by1 ? copyOffsets(b, bx0, bx1, period) : []);

        g.setTransform(dpr, 0, 0, dpr, 0, 0);
        g.fillStyle = colors.sea;
        g.fillRect(0, 0, bw, bh);
        g.setTransform(dpr * scale, 0, 0, dpr * scale, dpr * ox, dpr * oy);

        // 海峡与航线（在陆地之下，只露出海上部分）
        g.lineWidth = 1.2 / scale;
        g.setLineDash([4 / scale, 4 / scale]);
        g.strokeStyle = colors.seaLane;
        g.beginPath();
        for (const [a, b] of seaLinks) {
          const pa = centerOf(a);
          // 跨越 180° 经线的航线走较短的一侧
          const pb = nearest(pa, centerOf(b));
          for (const o of baseCopies(segmentBox(pa, pb))) {
            g.moveTo(pa[0] + o, pa[1]);
            g.lineTo(pb[0] + o, pb[1]);
          }
        }
        g.stroke();
        g.setLineDash([]);

        // 省份填色：按颜色合并路径，一种颜色只填一次；被占领时叠加法理所有者的斜线
        const fills = new Map<string, Path2D>();
        const hatches = new Map<string, Path2D>();
        const add = (groups: Map<string, Path2D>, color: string, path: Path2D, offset: number) => {
          let group = groups.get(color);
          if (!group) groups.set(color, (group = new Path2D()));
          group.addPath(path, shift(offset));
        };
        for (const [id, s] of geometry.shapes) {
          const controller = state.controllers[id];
          const owner = world.map.provinces[id].owner;
          const path = level.shapes.get(id)!;
          for (const o of baseCopies(s.bbox)) {
            add(fills, countryColor(world, controller), path, o);
            if (owner !== controller) add(hatches, countryColor(world, owner), path, o);
          }
        }
        // 合并后省份互不重叠、内飞地与外环的洞重合，evenodd 仍正确；节点图的圆可能重叠，改用 nonzero
        const rule = geometry.hasShapes ? "evenodd" : "nonzero";
        for (const [color, path] of fills) {
          g.fillStyle = color;
          g.fill(path, rule);
        }
        for (const [color, path] of hatches) {
          const pattern = hatch(g, color);
          pattern.setTransform(new DOMMatrix().scale(1 / scale));
          g.fillStyle = pattern;
          g.fill(path, rule);
        }

        if (!geometry.hasShapes) {
          g.lineWidth = 1.5 / scale;
          g.strokeStyle = colors.border;
          for (const path of fills.values()) g.stroke(path);
          return { canvas: out, view: current, key, at: now };
        }

        // 国界（含海岸线）、交战前线与省内边界，只取视野内的弧段
        const country = new Path2D();
        const front = new Path2D();
        const provinceLines = new Path2D();
        geometry.arcs.forEach((arc, i) => {
          const offsets = baseCopies(arc.bbox);
          if (!offsets.length) return;
          let target = country;
          if (arc.right !== null) {
            const a = state.controllers[arc.left];
            const b = state.controllers[arc.right];
            if (a === b) {
              if (!detail) return;
              target = provinceLines;
            } else if (state.countries[a]?.enemies.includes(b)) target = front;
          }
          for (const o of offsets) target.addPath(level.arcs[i], shift(o));
        });
        if (detail) {
          g.lineWidth = 0.6 / scale;
          g.strokeStyle = colors.provinceLine;
          g.stroke(provinceLines);
        }
        g.lineWidth = 1.4 / scale;
        g.strokeStyle = colors.border;
        g.stroke(country);
        g.lineWidth = 3 / scale;
        g.strokeStyle = colors.front;
        g.stroke(front);
        return { canvas: out, view: current, key, at: now };
      }

      // 贴上底图（缩放过程中按比例拉伸，稍后补绘清晰版本）
      const k = scale / b.view.scale;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.fillStyle = colors.sea;
      ctx.fillRect(0, 0, size.w, size.h);
      ctx.setTransform(k, 0, 0, k, dpr * (tx - b.view.tx * k - BASE_MARGIN * k), dpr * (ty - b.view.ty * k - BASE_MARGIN * k));
      ctx.drawImage(b.canvas, 0, 0);
      clearTimeout(refreshTimer.current);
      if (k !== 1) refreshTimer.current = window.setTimeout(requestPaint, ZOOM_REFRESH_MS);

      // 悬停与选中
      ctx.setTransform(dpr * scale, 0, 0, dpr * scale, dpr * tx, dpr * ty);
      if (hoverId !== null) {
        ctx.fillStyle = "rgba(255,255,255,0.18)";
        ctx.fill(copies(level.shapes.get(hoverId)!, geometry.shapes.get(hoverId)!.bbox), "evenodd");
      }
      ctx.lineWidth = 2.5 / scale;
      ctx.strokeStyle = colors.accent;
      ctx.stroke(copies(level.shapes.get(state.selected_province)!, geometry.shapes.get(state.selected_province)!.bbox));

      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      // 行军箭头
      for (const m of marches) {
        const from = centerOf(m.from);
        const to = nearest(from, centerOf(m.to));
        const color = countryColor(world, m.country);
        for (const o of visibleCopies(segmentBox(from, to))) {
          const a = toScreen(from, o);
          const b = toScreen(to, o);
          arrow(ctx, a, b, color, 2);
          const dot: Point = [a[0] + (b[0] - a[0]) * m.progress, a[1] + (b[1] - a[1]) * m.progress];
          ctx.fillStyle = color;
          ctx.strokeStyle = "#0008";
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.arc(dot[0], dot[1], 3.5, 0, Math.PI * 2);
          ctx.fill();
          ctx.stroke();
        }
      }

      // 部署预览
      if (targeting && hoverId !== null && hoverId !== targeting.source) {
        const from = centerOf(targeting.source);
        const to = nearest(from, centerOf(hoverId));
        for (const o of visibleCopies(segmentBox(from, to))) {
          arrow(ctx, toScreen(from, o), toScreen(to, o), colors.accent, 2.5, [6, 4]);
        }
      }

      // 微小省份（直布罗陀、马耳他……）画成可点击的圆点
      for (const [id, s] of geometry.shapes) {
        if (!geometry.hasShapes || !isTiny(s, scale)) continue;
        for (const o of visibleCopies(s.bbox)) {
          const [x, y] = toScreen(s.center, o);
          ctx.beginPath();
          ctx.arc(x, y, TINY_PX / 2, 0, Math.PI * 2);
          ctx.fillStyle = countryColor(world, state.controllers[id]);
          ctx.fill();
          ctx.lineWidth = id === state.selected_province ? 2 : 1;
          ctx.strokeStyle = id === state.selected_province ? colors.accent : colors.border;
          ctx.stroke();
        }
      }

      // 首都、地名
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.font = "11px sans-serif";
      for (const [id, s] of geometry.shapes) {
        for (const o of visibleCopies(s.bbox)) {
          const [x, y] = toScreen(s.center, o);
          if (capitals.has(id)) {
            ctx.fillStyle = colors.accent;
            ctx.fillText("★", x, y - 13);
          }
          const width = (s.bbox[2] - s.bbox[0]) * scale;
          const showName = geometry.hasShapes ? detail && width > 46 : true;
          if (showName && !unitCounts.has(id)) {
            ctx.lineWidth = 3;
            ctx.strokeStyle = "#000a";
            ctx.strokeText(world.map.provinces[id].name, x, y);
            ctx.fillStyle = "#f2efe6";
            ctx.fillText(world.map.provinces[id].name, x, y);
          }
        }
      }

      // 部队计数牌与交战标记
      ctx.font = "bold 11px monospace";
      for (const [id, counts] of unitCounts) {
        const s = geometry.shapes.get(id)!;
        for (const o of visibleCopies(s.bbox)) {
          const [x, y] = toScreen(s.center, o);
          const total = [...counts.values()].reduce((a, n) => a + n, 0);
          const [lead] = [...counts].sort((a, b) => b[1] - a[1])[0];
          const label = String(total);
          const w = Math.max(18, ctx.measureText(label).width + 8);
          ctx.fillStyle = countryColor(world, lead);
          ctx.strokeStyle = lead === state.player_country ? colors.accent : "#000c";
          ctx.lineWidth = lead === state.player_country ? 1.5 : 1;
          ctx.beginPath();
          ctx.roundRect(x - w / 2, y - 8, w, 16, 3);
          ctx.fill();
          ctx.stroke();
          ctx.fillStyle = "#fff";
          ctx.strokeStyle = "#000a";
          ctx.lineWidth = 2.5;
          ctx.strokeText(label, x, y + 0.5);
          ctx.fillText(label, x, y + 0.5);
          if (counts.size > 1) {
            // 多国部队同驻（交战中）以小色块提示
            [...counts.keys()].filter((c) => c !== lead).forEach((c, k) => {
              ctx.fillStyle = countryColor(world, c);
              ctx.fillRect(x + w / 2 + 1, y - 8 + k * 6, 4, 5);
            });
          }
        }
      }
      ctx.font = "12px sans-serif";
      for (const [id, combat] of combatAt) {
        const s = geometry.shapes.get(id)!;
        for (const o of visibleCopies(s.bbox)) {
          const [x, y] = toScreen(s.center, o);
          const barW = 26;
          ctx.fillStyle = countryColor(world, combat.defender_country);
          ctx.fillRect(x - barW / 2, y + 10, barW, 4);
          ctx.fillStyle = countryColor(world, combat.attacker_country);
          ctx.fillRect(x - barW / 2, y + 10, (barW * combat.progress) / 100, 4);
          ctx.strokeStyle = "#000c";
          ctx.lineWidth = 1;
          ctx.strokeRect(x - barW / 2, y + 10, barW, 4);
          ctx.fillStyle = colors.danger;
          ctx.fillText("⚔", x + 16, y - 10);
        }
      }
    };
    requestPaint();
  }, [view, size, state, world, geometry, controllersKey, enemiesKey, seaLinks, capitals, unitCounts, marches, combatAt, hoverId, colors, targeting, hatch, requestPaint]);

  /* ── 交互 ── */

  const pick = (e: { clientX: number; clientY: number }) => {
    const rect = canvasRef.current!.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    return { sx, sy, id: view ? hitTest(geometry, hitCtx, toWorld(sx, sy), view.scale) : null };
  };

  const onWheel = (e: React.WheelEvent) => {
    if (!view) return;
    const rect = canvasRef.current!.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    const factor = Math.exp(-e.deltaY * 0.0015);
    const scale = Math.min(fitScale * 60, Math.max(minScale, view.scale * factor));
    const k = scale / view.scale;
    commit({ scale, tx: sx - (sx - view.tx) * k, ty: sy - (sy - view.ty) * k });
  };

  // React 的 onWheel 是被动监听，无法阻止页面滚动
  useEffect(() => {
    const el = canvasRef.current!;
    const prevent = (e: WheelEvent) => e.preventDefault();
    el.addEventListener("wheel", prevent, { passive: false });
    return () => el.removeEventListener("wheel", prevent);
  }, []);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0 || !view) return;
    drag.current = { x: e.clientX, y: e.clientY, view, moved: false };
    canvasRef.current!.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (d) {
      const dx = e.clientX - d.x;
      const dy = e.clientY - d.y;
      if (!d.moved && Math.hypot(dx, dy) < DRAG_PX) return;
      d.moved = true;
      commit({ ...d.view, tx: d.view.tx + dx, ty: d.view.ty + dy });
      return;
    }
    const { sx, sy, id } = pick(e);
    setHover(id === null ? null : { id, x: sx, y: sy });
  };

  const onPointerUp = (e: React.PointerEvent) => {
    const d = drag.current;
    drag.current = null;
    if (d && !d.moved) {
      const { id } = pick(e);
      if (id !== null) onClick(id);
    }
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    const { id } = pick(e);
    if (id !== null) onQuickOrder(id, e.shiftKey);
  };

  return (
    <div className={`war-map${targeting ? " targeting" : ""}`} ref={wrapRef}>
      <canvas
        ref={canvasRef}
        style={{ width: size.w, height: size.h }}
        role="img"
        aria-label={`${world.scenario.name} 战略地图`}
        onWheel={onWheel}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => setHover(null)}
        onContextMenu={onContextMenu}
      />
      {hover && !drag.current && <Tooltip id={hover.id} x={hover.x} y={hover.y} state={state} world={world} targeting={targeting} counts={unitCounts.get(hover.id)} />}
      <div className="zoom-controls">
        <button onClick={() => view && commit(zoomAround(view, Math.min(1.5, (fitScale * 60) / view.scale), size))} aria-label="放大">
          +
        </button>
        <button onClick={() => view && commit(zoomAround(view, Math.max(1 / 1.5, minScale / view.scale), size))} aria-label="缩小">
          −
        </button>
      </div>
    </div>
  );
}

function zoomAround(view: View, k: number, size: { w: number; h: number }): View {
  const cx = size.w / 2;
  const cy = size.h / 2;
  return { scale: view.scale * k, tx: cx - (cx - view.tx) * k, ty: cy - (cy - view.ty) * k };
}

function Tooltip(props: {
  id: ProvinceId;
  x: number;
  y: number;
  state: GameState;
  world: World;
  targeting: Targeting;
  counts: Map<CountryId, number> | undefined;
}) {
  const { id, x, y, state, world, targeting, counts } = props;
  const p = world.map.provinces[id];
  const controller = state.controllers[id];
  const occupied = controller !== p.owner;
  const terrain = world.terrains[p.terrain]?.name ?? p.terrain;
  const relation =
    controller === state.player_country
      ? ""
      : state.countries[state.player_country].enemies.includes(controller)
        ? "（交战）"
        : areAllies(world, controller, state.player_country)
          ? "（盟友）"
          : "";
  return (
    <div className="map-tooltip" style={{ left: x + 14, top: y + 14 }}>
      <strong>{p.name}</strong>
      {p.name_en && <span className="muted"> {p.name_en}</span>}
      <div>
        {countryName(world, controller)}
        {relation}
        {occupied && <span className="warn"> · 占领自 {countryName(world, p.owner)}</span>}
      </div>
      <div className="muted">
        {terrain} · ★{p.victory_points}
        {counts && ` · 驻军 ${[...counts].map(([c, n]) => `${c}×${n}`).join(" ")}`}
      </div>
      <div className="muted">{targeting ? "点击：下达行军命令" : "右键：选中师团前往 · Shift+右键：全军突击"}</div>
    </div>
  );
}

/** 屏幕坐标下的箭头，两端让出计数牌的位置 */
function arrow(ctx: CanvasRenderingContext2D, a: Point, b: Point, color: string, width: number, dash: number[] = []) {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len = Math.hypot(dx, dy);
  if (len < 4) return;
  const ux = dx / len;
  const uy = dy / len;
  const inset = Math.min(12, len / 4);
  const [x1, y1] = [a[0] + ux * inset, a[1] + uy * inset];
  const [x2, y2] = [b[0] - ux * inset, b[1] - uy * inset];
  ctx.strokeStyle = color;
  ctx.fillStyle = color;
  ctx.lineWidth = width;
  ctx.setLineDash(dash);
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2, y2);
  ctx.stroke();
  ctx.setLineDash([]);
  const head = 7;
  ctx.beginPath();
  ctx.moveTo(x2, y2);
  ctx.lineTo(x2 - ux * head - uy * head * 0.6, y2 - uy * head + ux * head * 0.6);
  ctx.lineTo(x2 - ux * head + uy * head * 0.6, y2 - uy * head - ux * head * 0.6);
  ctx.closePath();
  ctx.fill();
}
