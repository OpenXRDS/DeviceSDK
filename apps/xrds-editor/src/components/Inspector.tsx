import { useState, useEffect, useRef, useCallback } from "react";
import type { EditorSnapshot, EditorCommand, NodeInspector, MaterialParams, EnvironmentDto } from "../types/bridge";
import { rgbaToHex, hexToRgba } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
  send:     (cmd: EditorCommand) => void;
}

// ---------------------------------------------------------------------------
// Scrub field — pointer-drag to change, click to type
// ---------------------------------------------------------------------------
interface ScrubProps {
  axis: "x"|"y"|"z";
  value: number;
  onLive: (v: number) => void;
  onCommit: (v: number) => void;
  step?: number;
}
function ScrubField({ axis, value, onLive, onCommit, step = 0.01 }: ScrubProps) {
  const [local, setLocal] = useState(value.toFixed(3));
  const pointerDown = useRef(false);
  const dragging    = useRef(false);
  const startX      = useRef(0);
  const startVal    = useRef(0);
  const wrapRef     = useRef<HTMLDivElement>(null);

  // Sync from outside only when not focused
  useEffect(() => {
    if (document.activeElement !== wrapRef.current?.querySelector("input")) {
      setLocal(value.toFixed(3));
    }
  }, [value]);

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    const input = wrapRef.current?.querySelector("input");
    if (document.activeElement === input) return;
    e.preventDefault();
    pointerDown.current = true; dragging.current = false;
    startX.current   = e.clientX;
    startVal.current = parseFloat(local) || 0;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }, [local]);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!pointerDown.current) return;
    const dx = e.clientX - startX.current;
    if (!dragging.current && Math.abs(dx) > 4) dragging.current = true;
    if (!dragging.current) return;
    const v = startVal.current + dx * step;
    setLocal(v.toFixed(3));
    onLive(v);
  }, [step, onLive]);

  const onPointerUp = useCallback((e: React.PointerEvent) => {
    if (!pointerDown.current) return;
    pointerDown.current = false;
    if (dragging.current) {
      onCommit(parseFloat(local) || 0);
      dragging.current = false;
    } else {
      const input = wrapRef.current?.querySelector("input") as HTMLInputElement | null;
      input?.focus(); input?.select();
    }
  }, [local, onCommit]);

  const axisColor = axis === "x" ? "ax-x" : axis === "y" ? "ax-y" : "ax-z";
  const axisLabel = axis.toUpperCase();

  return (
    <div className="scrub-wrap" ref={wrapRef}
         onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp}>
      <span className={`scrub-axis ${axisColor}`}>{axisLabel}</span>
      <input
        type="text"
        value={local}
        onChange={e => { setLocal(e.target.value); onLive(parseFloat(e.target.value) || 0); }}
        onBlur={e => onCommit(parseFloat(e.target.value) || 0)}
        onKeyDown={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); e.stopPropagation(); }}
      />
    </div>
  );
}

// T/R/S row
interface Vec3RowProps {
  rowClass: string; rowLabel: string;
  values: [number,number,number];
  onLive: (v: [number,number,number]) => void;
  onCommit: (v: [number,number,number]) => void;
  step?: number;
}
function Vec3Row({ rowClass, rowLabel, values, onLive, onCommit, step }: Vec3RowProps) {
  const cur = useRef<[number,number,number]>([...values]);
  // Keep cur in sync when values change externally
  useEffect(() => { cur.current = [...values]; }, [values]);

  return (
    <div className="tf-row">
      <span className={`tf-lbl ${rowClass}`}>{rowLabel}</span>
      {(["x","y","z"] as const).map((ax, i) => (
        <ScrubField key={ax} axis={ax} value={values[i]} step={step}
          onLive={v => { cur.current[i] = v; onLive([...cur.current]); }}
          onCommit={v => { cur.current[i] = v; onCommit([...cur.current]); }}
        />
      ))}
    </div>
  );
}

// Slider row
function SliderRow({ label, value, min, max, step, onLive, onCommit, disabled }: {
  label: string; value: number; min: number; max: number; step: number;
  onLive: (v: number) => void; onCommit: (v: number) => void; disabled?: boolean;
}) {
  const [local, setLocal] = useState(value);
  const [text, setText]   = useState(value.toFixed(step < 1 ? 2 : 0));
  useEffect(() => {
    setLocal(value);
    setText(value.toFixed(step < 1 ? 2 : 0));
  }, [value, step]);

  function applyText(raw: string) {
    if (disabled) return;
    const v = parseFloat(raw);
    if (isNaN(v)) { setText(local.toFixed(step < 1 ? 2 : 0)); return; }
    setLocal(v);
    setText(v.toFixed(step < 1 ? 2 : 0));
    onCommit(v);
  }

  return (
    <div className="insp-row" style={{ gap: 4, opacity: disabled ? 0.4 : 1 }}>
      <label>{label}</label>
      <input type="range" min={min} max={max} step={step}
        value={Math.min(max, Math.max(min, local))}  // clamp for display only
        style={{ flex: 1 }}
        disabled={disabled}
        onChange={e => { if (disabled) return; const v = +e.target.value; setLocal(v); setText(v.toFixed(step < 1 ? 2 : 0)); onLive(v); }}
        onMouseUp={e  => { if (!disabled) onCommit(+(e.target as HTMLInputElement).value); }}
      />
      {/* Direct number entry — accepts values outside slider range */}
      <input type="text" value={text}
        style={{ width: 52, background:"var(--surface0)", color:"var(--text)",
                 border:"1px solid var(--surface1)", borderRadius:3,
                 padding:"2px 4px", fontSize:11, fontFamily:"monospace",
                 flexShrink: 0 }}
        disabled={disabled}
        onChange={e => setText(e.target.value)}
        onBlur={e  => applyText(e.target.value)}
        onKeyDown={e => {
          e.stopPropagation();
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        }}
      />
    </div>
  );
}

// Color row
function ColorRow({ label, color, onLive, onCommit }: {
  label: string; color: [number,number,number,number];
  onLive: (c: [number,number,number,number]) => void;
  onCommit: (c: [number,number,number,number]) => void;
}) {
  return (
    <div className="insp-row">
      <label>{label}</label>
      <input type="color" value={rgbaToHex(color)}
        onChange={e => onLive(hexToRgba(e.target.value, color[3]))}
        onBlur={e => onCommit(hexToRgba((e.target as HTMLInputElement).value, color[3]))}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Inspector
// ---------------------------------------------------------------------------
export function Inspector({ snapshot, send }: Props) {
  const node = snapshot.selected_node;
  const prevId = useRef<number | null>(null);

  // Local state for transform (persists during scrubbing)
  const [tVal, setTVal] = useState<[number,number,number]>([0,0,0]);
  const [rVal, setRVal] = useState<[number,number,number]>([0,0,0]);
  const [sVal, setSVal] = useState<[number,number,number]>([1,1,1]);
  // True while a scrub drag is in progress — prevents snapshot from overwriting local state
  const isDragging = useRef(false);

  // Sync from snapshot whenever NOT dragging (covers both node change and gizmo commits)
  useEffect(() => {
    if (!node) { prevId.current = null; return; }
    const nodeChanged = node.id !== prevId.current;
    if (nodeChanged) prevId.current = node.id;
    if (nodeChanged || !isDragging.current) {
      setTVal([...node.translation]);
      setRVal([...node.rotation_euler_degrees]);
      setSVal([...node.scale]);
    }
  }, [node]);

  if (!node) {
    return (
      <div className="inspector">
        <div className="panel-header">Inspector</div>
        <SceneEnvironmentSection env={snapshot.environment} send={send} />
      </div>
    );
  }

  const id = node.id;
  const commitTf = (t: [number,number,number], r: [number,number,number], s: [number,number,number]) =>
    send({ type: "CommitTransform", payload: { id, translation: t, rotation_euler_degrees: r, scale: s } });

  return (
    <div className="inspector">
      <div className="panel-header">Inspector</div>

      {/* Node header */}
      <div className="insp-section">
        <div className="insp-name-row">
          <input type="text" key={node.id} defaultValue={node.name}
            onKeyDown={e => e.stopPropagation()}
            onBlur={e => send({ type: "RenameNode", payload: { id, name: e.target.value } })}
            onKeyUp={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          />
          <input type="checkbox" key={`vis-${node.id}`} defaultChecked={node.visible} title="Visible"
            style={{ accentColor: "var(--blue)", cursor: "pointer", width: 16, height: 16 }}
            onChange={e => send({ type: "SetVisible", payload: { id, visible: e.target.checked } })}
          />
          <input type="checkbox" key={`grab-${node.id}`} defaultChecked={node.grabbable} title="Grabbable (XR trigger)"
            style={{ accentColor: "var(--green)", cursor: "pointer", width: 16, height: 16 }}
            onChange={e => send({ type: "SetGrabbable", payload: { id, grabbable: e.target.checked } })}
          />
        </div>
        <div className="insp-kind">{node.payload.type}</div>
      </div>

      {/* Transform — hidden for HudText (head-locked) */}
      {node.payload.type !== "HudText" && <div className="insp-section">
        <h4>
          Transform
          {node.parent_id != null && (
            <span style={{ color: node.parent_kind === "PlayerAnchor" ? "var(--blue)" : "var(--overlay0)",
                           fontSize:9, fontWeight:"normal", marginLeft:6, letterSpacing:0 }}>
              {node.parent_kind === "PlayerAnchor" ? "anchor-local offset" : "local to parent"}
            </span>
          )}
        </h4>
        <Vec3Row rowClass="tf-t" rowLabel="T" values={tVal}
          onLive={v  => { isDragging.current = true;  setTVal(v); send({ type: "SetTranslation",   payload: { id, value: v } }); }}
          onCommit={v => { isDragging.current = false; setTVal(v); commitTf(v, rVal, sVal); }}
        />
        <Vec3Row rowClass="tf-r" rowLabel="R" values={rVal} step={0.5}
          onLive={v  => { isDragging.current = true;  setRVal(v); send({ type: "SetRotationEuler", payload: { id, degrees: v } }); }}
          onCommit={v => { isDragging.current = false; setRVal(v); commitTf(tVal, v, sVal); }}
        />
        <Vec3Row rowClass="tf-s" rowLabel="S" values={sVal}
          onLive={v  => { isDragging.current = true;  setSVal(v); send({ type: "SetScale",         payload: { id, value: v } }); }}
          onCommit={v => { isDragging.current = false; setSVal(v); commitTf(tVal, rVal, v); }}
        />
      </div>}

      {/* Payload-specific sections — key forces remount on node change so useState re-initialises */}
      <PayloadSection key={node.id} node={node} send={send} isPlaying={snapshot.is_playing} snapshot={snapshot} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Scene environment (shown when nothing is selected)
// ---------------------------------------------------------------------------

function Toggle({ label, value, onChange }: { label: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="insp-row">
      <label>{label}</label>
      <input type="checkbox" checked={value} style={{ accentColor:"var(--blue)", cursor:"pointer", width:16, height:16 }}
        onChange={e => onChange(e.target.checked)} />
    </div>
  );
}

function SceneEnvironmentSection({ env, send }: { env: EnvironmentDto | null; send: (c: EditorCommand) => void }) {
  // Bevy's default ev100 is 9.7 (outdoor daylight). We display exposure as a
  // "brightness" offset where 0 = Bevy default, positive = brighter, negative = darker.
  // Mapping: displayed_brightness = BEVY_EV100 - stored_ev100  →  brighter = lower ev100.
  const BEVY_EV100 = 9.7;
  const toBrightness = (ev: number) => BEVY_EV100 - ev;
  const toEv100 = (b: number) => BEVY_EV100 - b;

  const e = env ?? { fog_enabled:false, fog_color:[1,0.4,0.1,1] as [number,number,number,number], fog_start:2, fog_end:30,
                     exposure_enabled:false, ev100:BEVY_EV100,
                     ibl_enabled:false, ibl_diffuse:"", ibl_specular:"", ibl_intensity:1,
                     skybox_enabled:false, skybox_asset:"", skybox_brightness:1 };

  const [fogColor, setFogColor]     = useState<[number,number,number,number]>(e.fog_color);
  const [fogStart, setFogStart]     = useState(e.fog_start);
  const [fogEnd,   setFogEnd]       = useState(e.fog_end);
  // brightness = BEVY_EV100 - ev100  (0 = default, positive = brighter)
  const [brightness, setBrightness] = useState(toBrightness(e.ev100));
  const isDragging = useRef(false);

  // Sync only when env CONTENT changes and user isn't dragging.
  // JSON.stringify prevents the effect from firing every 16 ms on the same values.
  const envKey = JSON.stringify(env);
  useEffect(() => {
    if (isDragging.current) return;
    if (env) {
      setFogColor(env.fog_color); setFogStart(env.fog_start);
      setFogEnd(env.fog_end);     setBrightness(toBrightness(env.ev100));
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [envKey]);

  return (
    <>
      <div className="insp-empty" style={{ fontSize:10, color:"var(--overlay0)", padding:"8px 10px 2px" }}>
        Select a node to inspect it
      </div>

      {/* Fog */}
      <div className="insp-section">
        <h4>Fog</h4>
        <Toggle label="Enable" value={e.fog_enabled}
          onChange={on => on
            ? send({ type:"SetFog", payload:{ color:fogColor, start:fogStart, end:fogEnd } })
            : send({ type:"ClearFog" })} />
        {e.fog_enabled && <>
          <ColorRow label="Color" color={fogColor}
            onLive={v  => { isDragging.current = true;  setFogColor(v); }}
            onCommit={v => { isDragging.current = false; setFogColor(v); send({ type:"SetFog", payload:{ color:v, start:fogStart, end:fogEnd } }); }} />
          <SliderRow label="Start" value={fogStart} min={0} max={500} step={1}
            onLive={v  => { isDragging.current = true;  setFogStart(v); }}
            onCommit={v => { isDragging.current = false; setFogStart(v); send({ type:"SetFog", payload:{ color:fogColor, start:v, end:fogEnd } }); }} />
          <SliderRow label="End" value={fogEnd} min={1} max={2000} step={1}
            onLive={v  => { isDragging.current = true;  setFogEnd(v); }}
            onCommit={v => { isDragging.current = false; setFogEnd(v); send({ type:"SetFog", payload:{ color:fogColor, start:fogStart, end:v } }); }} />
        </>}
      </div>

      {/* Exposure */}
      <div className="insp-section">
        <h4>Exposure</h4>
        <Toggle label="Enable" value={e.exposure_enabled}
          onChange={on => on ? send({ type:"SetExposure", payload:{ ev100: toEv100(brightness) } }) : send({ type:"ClearExposure" })} />
        {e.exposure_enabled && (
          // Display as "Brightness" offset: 0 = Bevy default (ev100=9.7), +5 = brighter, -5 = darker
          <SliderRow label="Brightness" value={brightness} min={-5} max={5} step={0.1}
            onLive={v  => { isDragging.current = true;  setBrightness(v); }}
            onCommit={v => { isDragging.current = false; setBrightness(v); send({ type:"SetExposure", payload:{ ev100: toEv100(v) } }); }} />
        )}
      </div>

      {/* IBL */}
      <div className="insp-section">
        <h4>IBL <span style={{color:"var(--overlay0)",fontSize:9,fontWeight:"normal",marginLeft:4}}>image-based lighting</span></h4>
        <Toggle label="Enable" value={e.ibl_enabled}
          onChange={on => on
            ? send({ type:"SetIbl", payload:{ diffuse_asset_id:e.ibl_diffuse, specular_asset_id:e.ibl_specular, intensity:e.ibl_intensity } })
            : send({ type:"ClearIbl" })} />
        {e.ibl_enabled && (
          <div className="insp-note">Set diffuse/specular asset IDs from imported environment maps.</div>
        )}
      </div>
    </>
  );
}

function PayloadSection({ node, send, isPlaying, snapshot }: { node: NodeInspector; send: (c: EditorCommand) => void; isPlaying: boolean; snapshot: EditorSnapshot }) {
  const { id, payload } = node;

  // Tetrahedron is mapped to Cube DTO on the Rust side
  if (payload.type === "Cube" || payload.type === "Sphere" || payload.type === "Cylinder" ||
      payload.type === "Plane") {
    return <PrimitiveSection id={id} mat={payload.material} physics_body={payload.physics_body} gravity_scale={payload.gravity_scale} mass={payload.mass} send={send} />;
  }
  if (payload.type === "PointLight") {
    return <PointLightSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "DirectionalLight") {
    return <DirLightSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "AmbientLight") {
    return <AmbientSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "SpotLight") {
    return <SpotLightSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "Camera") {
    return <CameraSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "GltfAsset") {
    return <GltfSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "HudText") {
    return <HudTextSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "Text") {
    return <TextSection id={id} p={payload} parentKind={node.parent_kind} send={send} />;
  }
  if (payload.type === "ExtrudedText") {
    return <ExtrudedTextSection id={id} p={payload} send={send} />;
  }
  if (payload.type === "Player") {
    return null;
  }
  if (payload.type === "PlayerAnchor") {
    return <PlayerAnchorSection id={id} p={payload} send={send} isPlaying={isPlaying} snapshot={snapshot} />;
  }
  if (payload.type === "PlayerSpawnZone") {
    return <SpawnZoneSection id={id} p={payload} send={send} snapshot={snapshot} />;
  }
  return null;
}

const PHYSICS_BODY_OPTIONS = ["None", "Static", "Dynamic"] as const;

function PrimitiveSection({ id, mat, physics_body, gravity_scale, mass, send }: { id: number; mat: MaterialParams; physics_body: string; gravity_scale: number; mass: number; send: (c: EditorCommand) => void }) {
  const [local, setLocal] = useState<MaterialParams>(mat);
  const isDragging = useRef(false);
  useEffect(() => { if (!isDragging.current) setLocal(mat); }, [mat]);
  const upd    = (m: MaterialParams) => { isDragging.current = true;  send({ type: "SetMaterial",   payload: { id, params: m } }); };
  const commit = (m: MaterialParams) => { isDragging.current = false; send({ type: "CommitMaterial", payload: { id, params: m } }); };
  const isDynamic = physics_body === "Dynamic";
  return (
    <div className="insp-section">
      <h4>Physics</h4>
      <div className="insp-row">
        <span className="insp-label">Body</span>
        <select value={physics_body} onChange={e => send({ type: "SetPhysicsBody", payload: { id, physics_body: e.target.value } })}>
          {PHYSICS_BODY_OPTIONS.map(o => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {isDynamic && <>
        <SliderRow label="Gravity Scale" value={gravity_scale} min={0} max={3} step={0.01}
          onLive={v  => send({ type: "SetGravityScale", payload: { id, value: v } })}
          onCommit={v => send({ type: "SetGravityScale", payload: { id, value: v } })}
        />
        <SliderRow label="Mass (kg)" value={mass} min={0.01} max={100} step={0.01}
          onLive={v  => send({ type: "SetMass", payload: { id, value: v } })}
          onCommit={v => send({ type: "SetMass", payload: { id, value: v } })}
        />
      </>}
      <h4>Material</h4>
      <ColorRow label="Base Color" color={local.base_color}
        onLive={c => { const m = {...local, base_color: c}; setLocal(m); upd(m); }}
        onCommit={c => { const m = {...local, base_color: c}; setLocal(m); commit(m); }}
      />
      <SliderRow label="Metallic" value={local.metallic} min={0} max={1} step={0.01}
        onLive={v => { const m = {...local, metallic: v}; setLocal(m); upd(m); }}
        onCommit={v => { const m = {...local, metallic: v}; setLocal(m); commit(m); }}
      />
      <SliderRow label="Roughness" value={local.roughness} min={0} max={1} step={0.01}
        onLive={v => { const m = {...local, roughness: v}; setLocal(m); upd(m); }}
        onCommit={v => { const m = {...local, roughness: v}; setLocal(m); commit(m); }}
      />
    </div>
  );
}

function MaterialSection({ id, mat, send }: { id: number; mat: MaterialParams; send: (c: EditorCommand) => void }) {
  const [local, setLocal] = useState<MaterialParams>(mat);
  const isDragging = useRef(false);
  // Sync from snapshot only when not dragging (prevents overwrite during slider drag)
  useEffect(() => { if (!isDragging.current) setLocal(mat); }, [mat]);
  const upd    = (m: MaterialParams) => { isDragging.current = true;  send({ type: "SetMaterial",   payload: { id, params: m } }); };
  const commit = (m: MaterialParams) => { isDragging.current = false; send({ type: "CommitMaterial", payload: { id, params: m } }); };
  return (
    <div className="insp-section">
      <h4>Material</h4>
      <ColorRow label="Base Color" color={local.base_color}
        onLive={c => { const m = {...local, base_color: c}; setLocal(m); upd(m); }}
        onCommit={c => { const m = {...local, base_color: c}; setLocal(m); commit(m); }}
      />
      <SliderRow label="Metallic" value={local.metallic} min={0} max={1} step={0.01}
        onLive={v => { const m = {...local, metallic: v}; setLocal(m); upd(m); }}
        onCommit={v => { const m = {...local, metallic: v}; setLocal(m); commit(m); }}
      />
      <SliderRow label="Roughness" value={local.roughness} min={0} max={1} step={0.01}
        onLive={v => { const m = {...local, roughness: v}; setLocal(m); upd(m); }}
        onCommit={v => { const m = {...local, roughness: v}; setLocal(m); commit(m); }}
      />
    </div>
  );
}

function PointLightSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [c, setC] = useState(p.color); const [intensity, setI] = useState(p.intensity); const [range, setR] = useState(p.range);
  const upd    = (color: any, i: number, r: number) => send({ type: "SetPointLight",  payload: { id, color, intensity: i, range: r } });
  const commit = () => send({ type: "CommitLight", payload: { id } });
  return (
    <div className="insp-section">
      <h4>Point Light</h4>
      <ColorRow label="Color" color={c} onLive={v => { setC(v); upd(v, intensity, range); }} onCommit={v => { setC(v); upd(v, intensity, range); commit(); }} />
      <SliderRow label="Intensity" value={intensity} min={0} max={100000} step={100} onLive={v => { setI(v); upd(c, v, range); }} onCommit={v => { setI(v); upd(c, v, range); commit(); }} />
      <SliderRow label="Range"     value={range}     min={0} max={100}    step={0.1} onLive={v => { setR(v); upd(c, intensity, v); }} onCommit={v => { setR(v); upd(c, intensity, v); commit(); }} />
      <div className="insp-note">ℹ Light visible when geometry is nearby.</div>
    </div>
  );
}

function DirLightSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [c, setC] = useState(p.color); const [lux, setL] = useState(p.illuminance);
  const upd    = (color: any, illuminance: number) => send({ type: "SetDirectionalLight", payload: { id, color, illuminance } });
  const commit = () => send({ type: "CommitLight", payload: { id } });
  return (
    <div className="insp-section">
      <h4>Directional Light</h4>
      <ColorRow label="Color" color={c} onLive={v => { setC(v); upd(v, lux); }} onCommit={v => { setC(v); upd(v, lux); commit(); }} />
      <SliderRow label="Illuminance" value={lux} min={0} max={150000} step={500} onLive={v => { setL(v); upd(c, v); }} onCommit={v => { setL(v); upd(c, v); commit(); }} />
    </div>
  );
}

function AmbientSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [c, setC] = useState(p.color); const [brightness, setB] = useState(p.brightness);
  const upd    = (color: any, b: number) => send({ type: "SetAmbientLight", payload: { id, color, brightness: b } });
  const commit = () => send({ type: "CommitLight", payload: { id } });
  return (
    <div className="insp-section">
      <h4>Ambient Light</h4>
      <ColorRow label="Color" color={c} onLive={v => { setC(v); upd(v, brightness); }} onCommit={v => { setC(v); upd(v, brightness); commit(); }} />
      <SliderRow label="Brightness" value={brightness} min={0} max={2000} step={10} onLive={v => { setB(v); upd(c, v); }} onCommit={v => { setB(v); upd(c, v); commit(); }} />
    </div>
  );
}

function SpotLightSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [c, setC] = useState(p.color); const [i, setI] = useState(p.intensity);
  const [r, setR] = useState(p.range); const [inn, setInn] = useState(p.inner_angle); const [out, setOut] = useState(p.outer_angle);
  const upd    = (color: any, intensity: number, range: number, inner: number, outer: number) =>
    send({ type: "SetSpotLight", payload: { id, color, intensity, range, inner_angle: inner, outer_angle: outer } });
  const commit = () => send({ type: "CommitLight", payload: { id } });
  return (
    <div className="insp-section">
      <h4>Spot Light</h4>
      <ColorRow label="Color" color={c} onLive={v => { setC(v); upd(v,i,r,inn,out); }} onCommit={v => { setC(v); upd(v,i,r,inn,out); commit(); }} />
      <SliderRow label="Intensity" value={i} min={0} max={100000} step={100} onLive={v => { setI(v); upd(c,v,r,inn,out); }} onCommit={v => { setI(v); upd(c,v,r,inn,out); commit(); }} />
      <SliderRow label="Range"     value={r} min={0} max={100}    step={0.1} onLive={v => { setR(v); upd(c,i,v,inn,out); }} onCommit={v => { setR(v); upd(c,i,v,inn,out); commit(); }} />
    </div>
  );
}

function CameraSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [fov, setFov]   = useState<number>(p.fov);
  const [near, setNear] = useState<number>(p.near);
  const [far, setFar]   = useState<number>(p.far);

  useEffect(() => { setFov(p.fov); setNear(p.near); setFar(p.far); }, [p.fov, p.near, p.far]);

  const sendLive   = (f: number) => send({ type: "SetCameraParams",    payload: { id, fov: f, near, far } });
  const sendCommit = (f: number, n: number, fa: number) =>
    send({ type: "CommitCameraParams", payload: { id, fov: f, near: n, far: fa } });

  return (
    <div className="insp-section">
      <h4>Camera</h4>
      <SliderRow label="FOV"  value={fov}  min={10}  max={170} step={0.5}
        onLive={v => { setFov(v);  sendLive(v); }}
        onCommit={v => { setFov(v);  sendCommit(v, near, far); }} />
      <SliderRow label="Near" value={near} min={0.01} max={10}  step={0.01}
        onLive={v => { setNear(v); }}
        onCommit={v => { setNear(v); sendCommit(fov, v, far); }} />
      <SliderRow label="Far"  value={far}  min={10}  max={5000} step={1}
        onLive={v => { setFar(v); }}
        onCommit={v => { setFar(v);  sendCommit(fov, near, v); }} />
    </div>
  );
}

const HUD_ANCHORS = ["TopLeft","TopCenter","TopRight","MiddleLeft","Center","MiddleRight","BottomLeft","BottomCenter","BottomRight"];

function HudTextSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [text, setText]     = useState(p.text);
  const [size, setSize]     = useState(p.font_size);
  const [col, setCol]       = useState(p.color);
  const [anchor, setAnchor] = useState(p.anchor);
  const [offset, setOffset] = useState<[number,number]>(p.offset);

  const commit = () => send({ type: "SetHudText", payload: { id, text, font_size: size, color: col, anchor, offset } });

  return (
    <div className="insp-section">
      <h4>HUD Text <span style={{color:"var(--overlay0)", fontSize:9, fontWeight:"normal", marginLeft:4}}>screen-space</span></h4>

      <div className="insp-row">
        <label>Text</label>
        <input type="text" value={text} className="full-input"
          onKeyDown={e => e.stopPropagation()}
          onChange={e => setText(e.target.value)}
          onBlur={commit}
          onKeyUp={e => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
      </div>

      <SliderRow label="Font Size" value={size} min={8} max={96} step={1}
        onLive={v => setSize(v)} onCommit={v => { setSize(v); commit(); }} />

      <ColorRow label="Color" color={col}
        onLive={v => setCol(v)} onCommit={v => { setCol(v); commit(); }} />

      <div className="insp-row">
        <label>Anchor</label>
        <select value={anchor} className="full-input"
          onChange={e => {
            const a = e.target.value;
            setAnchor(a);
            // Send immediately with the new value — React state is async so
            // commit() would still see the old anchor if called here.
            send({ type: "SetHudText", payload: { id, text, font_size: size, color: col, anchor: a, offset } });
          }}>
          {HUD_ANCHORS.map(a => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>

      {/* Pixel offset from anchor */}
      <div style={{ marginTop: 4 }}>
        <div className="tf-row">
          <span className="insp-tf-lbl" style={{ width: 28, color:"var(--overlay0)", fontSize:10 }}>Off</span>
          {(["ax-x","ax-y"] as const).map((ax, i) => (
            <ScrubField key={ax} axis={ax === "ax-x" ? "x" : "y"} value={offset[i]} step={1}
              onLive={v => { const o: [number,number] = [...offset]; o[i] = v; setOffset(o); }}
              onCommit={v => { const o: [number,number] = [...offset]; o[i] = v; setOffset(o); commit(); }}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function GltfSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const clips: { index: number; name: string }[] = p.clips ?? [];
  const [clipIndex, setClipIndex] = useState(0);
  const [speed, setSpeed] = useState(1.0);
  return (
    <div className="insp-section">
      <h4>GLTF Asset</h4>
      {clips.length > 0 ? (
        <div className="insp-row">
          <label>Clip</label>
          <select value={clipIndex} onChange={e => setClipIndex(+e.target.value)}>
            {clips.map(cl => <option key={cl.index} value={cl.index}>{cl.name || `Clip ${cl.index}`}</option>)}
          </select>
        </div>
      ) : (
        <div className="insp-row"><label>Clip</label><span style={{color:"var(--overlay0)", fontSize:11}}>No clips</span></div>
      )}
      <SliderRow label="Speed" value={speed} min={0.1} max={4} step={0.1} onLive={v => setSpeed(v)} onCommit={v => setSpeed(v)} />
      <div style={{ display:"flex", gap:6, marginTop:4 }}>
        <button className="tb-btn" onClick={() => send({ type:"PlayGltfAnimation", payload:{ id, clip_index: clipIndex, speed, repeat:"Loop" } })}>▶ Play</button>
        <button className="tb-btn" onClick={() => send({ type:"StopGltfAnimation", payload:{ id } })}>■ Stop</button>
      </div>
    </div>
  );
}

function collectByKind(nodes: import("../types/bridge").HierarchyNode[], kind: string): { id: number; name: string }[] {
  const result: { id: number; name: string }[] = [];
  for (const n of nodes) {
    if (n.kind === kind) result.push({ id: n.id, name: n.name });
    result.push(...collectByKind(n.children, kind));
  }
  return result;
}

function SpawnZoneSection({ id, p, send, snapshot }: { id: number; p: any; send: (c: EditorCommand) => void; snapshot: EditorSnapshot }) {
  const [size, setSize] = useState<[number, number, number]>(p.size ?? [4.0, 0.1, 4.0]);
  useEffect(() => { setSize(p.size ?? [4.0, 0.1, 4.0]); }, [p.size]);

  const playerNodeId: number | null = p.player_node_id ?? null;
  const playerNodes = collectByKind(snapshot.hierarchy, "Player");

  const commit = (s: [number, number, number]) =>
    send({ type: "SetSpawnZoneSize", payload: { id, size: s } });

  return (
    <div className="insp-section">
      <h4>Spawn Zone <span style={{ color: "var(--overlay0)", fontSize: 9, fontWeight: "normal", marginLeft: 4 }}>W × H × D (metres)</span></h4>
      <SliderRow label="Width"  value={size[0]} min={0.1} max={50} step={0.1}
        onLive={v  => setSize([v, size[1], size[2]])}
        onCommit={v => { const s: [number,number,number] = [v, size[1], size[2]]; setSize(s); commit(s); }} />
      <SliderRow label="Height" value={size[1]} min={0.01} max={10} step={0.01}
        onLive={v  => setSize([size[0], v, size[2]])}
        onCommit={v => { const s: [number,number,number] = [size[0], v, size[2]]; setSize(s); commit(s); }} />
      <SliderRow label="Depth"  value={size[2]} min={0.1} max={50} step={0.1}
        onLive={v  => setSize([size[0], size[1], v])}
        onCommit={v => { const s: [number,number,number] = [size[0], size[1], v]; setSize(s); commit(s); }} />

      <div className="insp-row" style={{ marginTop: 6 }}>
        <label>Player</label>
        <select
          value={playerNodeId ?? ""}
          style={{ flex: 1, fontSize: 11 }}
          onChange={e => {
            const val = e.target.value;
            send({ type: "SetSpawnZonePlayer", payload: { id, player_node_id: val === "" ? null : Number(val) } });
          }}
        >
          <option value="">— shared (any player) —</option>
          {playerNodes.map(p => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>
      </div>
      {playerNodeId !== null && playerNodes.find(p => p.id === playerNodeId) == null && (
        <div className="insp-note" style={{ color: "var(--red)" }}>
          ⚠ Designated player node was deleted or renamed
        </div>
      )}
      <div className="insp-note">
        Players teleport to a random XZ position within this box on load.
      </div>
    </div>
  );
}

function PlayerAnchorSection({ id, p, send, isPlaying, snapshot }: { id: number; p: any; send: (c: EditorCommand) => void; isPlaying: boolean; snapshot: EditorSnapshot }) {
  const BEVY_EV100 = 9.7;
  const toBrightness = (ev: number) => BEVY_EV100 - ev;
  const toEv100 = (b: number) => BEVY_EV100 - b;

  const [fov, setFov] = useState<number>(p.fov_deg ?? 60);
  // exposure: null = inherit scene-wide; number = override (stored as ev100)
  const [expEnabled, setExpEnabled] = useState<boolean>(p.exposure != null);
  const [brightness, setBrightness] = useState<number>(p.exposure != null ? toBrightness(p.exposure) : 0);

  useEffect(() => { setFov(p.fov_deg ?? 60); }, [p.fov_deg]);
  useEffect(() => {
    setExpEnabled(p.exposure != null);
    setBrightness(p.exposure != null ? toBrightness(p.exposure) : 0);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [p.exposure]);

  const hudTemplateId: number | null = p.hud_template_id ?? null;
  const templates = snapshot.hud_library;

  return (
    <div className="insp-section">
      <h4>Player Anchor</h4>
      <SliderRow label="FOV (°)" value={fov} min={10} max={170} step={1}
        disabled={isPlaying}
        onLive={v => { if (!isPlaying) { setFov(v); send({ type: "SetPlayerAnchorFov", payload: { id, fov_deg: v } }); } }}
        onCommit={v => { if (!isPlaying) { setFov(v); send({ type: "SetPlayerAnchorFov", payload: { id, fov_deg: v } }); } }} />
      {isPlaying && (
        <div style={{ fontSize: 10, color: "var(--overlay0)", marginBottom: 4 }}>
          FOV applies on next anchor switch
        </div>
      )}
      <Toggle label="Initial spawn anchor" value={p.is_initial ?? false}
        onChange={v => send({ type: "SetPlayerAnchorInitial", payload: { id, is_initial: v } })} />

      {/* Per-anchor exposure override */}
      <div style={{ marginTop: 8 }}>
        <Toggle label="Override exposure" value={expEnabled}
          onChange={on => {
            setExpEnabled(on);
            send({ type: "SetPlayerAnchorExposure", payload: { id, ev100: on ? toEv100(brightness) : null } });
          }} />
        {expEnabled && (
          <SliderRow label="Brightness" value={brightness} min={-5} max={5} step={0.1}
            onLive={v  => setBrightness(v)}
            onCommit={v => { setBrightness(v); send({ type: "SetPlayerAnchorExposure", payload: { id, ev100: toEv100(v) } }); }} />
        )}
        {expEnabled && (
          <div className="insp-note">Overrides scene exposure when this anchor is active.</div>
        )}
      </div>

      <div className="insp-row" style={{ marginTop: 8 }}>
        <label>HUD Template</label>
        <select
          value={hudTemplateId ?? ""}
          style={{ flex: 1, fontSize: 11 }}
          onChange={e => {
            const val = e.target.value;
            send({ type: "LinkHudTemplate", payload: { anchor_id: id, template_id: val === "" ? null : Number(val) } });
          }}
        >
          <option value="">— none —</option>
          {templates.map(t => (
            <option key={t.id} value={t.id}>{t.name}</option>
          ))}
        </select>
      </div>
      {hudTemplateId !== null && templates.find(t => t.id === hudTemplateId) == null && (
        <div className="insp-note" style={{ color: "var(--red)" }}>
          ⚠ Linked template was deleted
        </div>
      )}
    </div>
  );
}

const TEXT_ANCHORS = ["World","Billboard","HeadLocked","BodyLocked","ComfortPinned","Cylindrical"];
const CAMERA_RELATIVE_ANCHORS = new Set(["HeadLocked","BodyLocked","ComfortPinned","Cylindrical"]);

function TextSection({ id, p, parentKind, send }: { id: number; p: any; parentKind?: string | null; send: (c: EditorCommand) => void }) {
  const [text, setText]     = useState(p.text);
  const [size, setSize]     = useState(p.font_size);
  const [col, setCol]       = useState(p.color);
  const [align, setAlign]   = useState(p.alignment);
  const [anchor, setAnchor] = useState<string>(p.anchor ?? "World");
  const [anchorParam, setAnchorParam] = useState<number>(p.anchor_param ?? 1.0);

  const commit = (overrides?: Partial<{ align: string; anchor: string; anchor_param: number }>) =>
    send({ type: "SetTextContent", payload: {
      id, text, font_size: size, color: col,
      alignment:    overrides?.align        ?? align,
      anchor:       overrides?.anchor       ?? anchor,
      anchor_param: overrides?.anchor_param ?? anchorParam,
    }});

  return (
    <div className="insp-section">
      <h4>Text</h4>
      <div className="insp-row"><label>Text</label>
        <input type="text" value={text}
          onChange={e => setText(e.target.value)} onBlur={() => commit()}
          onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
      </div>
      <SliderRow label="Font Size" value={size} min={4} max={200} step={0.5} onLive={v => setSize(v)} onCommit={v => { setSize(v); commit(); }} />
      <ColorRow label="Color" color={col} onLive={v => setCol(v)} onCommit={v => { setCol(v); commit(); }} />
      <div className="insp-row"><label>Align</label>
        <select value={align} onChange={e => {
          const a = e.target.value; setAlign(a); commit({ align: a });
        }}>
          {["Left","Center","Right"].map(a => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
      <div className="insp-row"><label>Anchor</label>
        <select value={anchor} onChange={e => {
          const a = e.target.value;
          // When switching to a param-bearing anchor with no prior param, seed a visible default.
          const needsParam = a === "ComfortPinned" || a === "Cylindrical";
          const newParam = needsParam && anchorParam < 0.05 ? 1.0 : anchorParam;
          setAnchor(a);
          if (newParam !== anchorParam) setAnchorParam(newParam);
          commit({ anchor: a, anchor_param: newParam });
        }}>
          {TEXT_ANCHORS.map(a => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
      {(anchor === "ComfortPinned") && (
        <SliderRow label="Depth (m)" value={anchorParam} min={0.1} max={10} step={0.05}
          onLive={v => setAnchorParam(v)}
          onCommit={v => { setAnchorParam(v); commit({ anchor_param: v }); }} />
      )}
      {(anchor === "Cylindrical") && (
        <SliderRow label="Radius (m)" value={anchorParam} min={0.1} max={10} step={0.05}
          onLive={v => setAnchorParam(v)}
          onCommit={v => { setAnchorParam(v); commit({ anchor_param: v }); }} />
      )}
      {CAMERA_RELATIVE_ANCHORS.has(anchor) && parentKind === "Player" && (
        <div style={{ marginTop:6, padding:"5px 7px", background:"rgba(250,179,135,0.12)",
                      border:"1px solid var(--peach)", borderRadius:3, fontSize:11,
                      color:"var(--peach)", lineHeight:1.4 }}>
          ⚠ Camera-relative text must be under a <strong>PlayerAnchor</strong>, not Player.
          Move this node under a PlayerAnchor child — it will not follow the camera at runtime.
        </div>
      )}
    </div>
  );
}

function ExtrudedTextSection({ id, p, send }: { id: number; p: any; send: (c: EditorCommand) => void }) {
  const [text, setText] = useState(p.text); const [size, setSize] = useState(p.font_size);
  const [depth, setDepth] = useState(p.depth); const [col, setCol] = useState(p.color);
  const [align, setAlign] = useState(p.alignment);
  const commit = () => send({ type: "SetExtrudedText", payload: { id, text, font_size: size, depth, color: col, alignment: align } });
  return (
    <div className="insp-section">
      <h4>Extruded Text</h4>
      <div className="insp-row"><label>Text</label>
        <input type="text" value={text}
          onChange={e => setText(e.target.value)} onBlur={commit}
          onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
      </div>
      <SliderRow label="Font Size" value={size}  min={4}   max={200} step={0.5}  onLive={v => setSize(v)}  onCommit={v => { setSize(v);  commit(); }} />
      <SliderRow label="Depth"     value={depth} min={0.01} max={5}  step={0.05} onLive={v => setDepth(v)} onCommit={v => { setDepth(v); commit(); }} />
      <ColorRow label="Color" color={col}
        onLive={v => { setCol(v); send({ type: "SetExtrudedTextColor", payload: { id, color: v } }); }}
        onCommit={v => { setCol(v); commit(); }}
      />
      <div className="insp-row"><label>Align</label>
        <select value={align} onChange={e => {
          const a = e.target.value; setAlign(a);
          send({ type:"SetExtrudedText", payload:{ id, text, font_size:size, depth, color:col, alignment:a } });
        }}>
          {["Left","Center","Right"].map(a => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
    </div>
  );
}
