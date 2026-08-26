import {
  AppWindow, Box, Camera, Circle, CircleDashed, CloudSun, Cylinder, Flashlight,
  Gamepad2, Hexagon, Image, Lightbulb, MapPin, Mountain, Music, Package, PanelTop,
  PersonStanding, Pill, RectangleHorizontal, Sparkles, SquareDashed, Sun, Triangle,
  Type, Video, Wind, Zap,
  type LucideIcon,
} from "lucide-react";

/** Node- and asset-kind icons.
 *
 * These were emoji. Emoji are drawn by the *operating system's* font — Segoe UI
 * Emoji on Windows, Noto Color Emoji on Linux — so the same build looked like a
 * different editor on each, and this one ships on both. They are also colour
 * glyphs: they ignore the theme, cannot dim with a disabled row, and cannot take a
 * lane's tint. Half the old set (`▶ ■ ⚠`) were plain text symbols that *did* theme,
 * so the icons disagreed with each other about whether they were themeable at all.
 *
 * Lucide icons are stroke SVG with `stroke="currentColor"`, so an icon is simply
 * the colour of whatever it sits in, and `size` is a prop rather than a font size.
 */
const KIND_ICONS: Record<string, LucideIcon> = {
  // Geometry. Lucide has the awkward ones — Cylinder and a capsule (Pill) — which
  // is why it was chosen over Radix Icons, whose set stops at box and circle.
  Cube: Box,
  Sphere: Circle,
  Cylinder,
  Capsule: Pill,
  Plane: RectangleHorizontal,
  Tetrahedron: Triangle,

  Camera,
  // Sun / bulb / torch reads as directional / point / spot without a legend.
  DirectionalLight: Sun,
  PointLight: Lightbulb,
  SpotLight: Flashlight,
  AmbientLight: CloudSun,

  GltfAsset: Package,
  Text: Type,
  ExtrudedText: Type,
  Billboard: Image,
  HudText: PanelTop,
  AudioClip: Music,
  InteractionZone: Hexagon,
  PlayerSpawn: PersonStanding,
  PlayerSpawnZone: SquareDashed,
  Player: Gamepad2,
  PlayerAnchor: MapPin,

  Effect: Sparkles,
  EffectBurst: Zap,
  EffectTrail: Wind,

  Panel: AppWindow,
  Empty: CircleDashed,

  // Asset-catalog kinds, which share this table because they name the same things
  // from the other side — an imported `Gltf` and a `GltfAsset` node are one idea.
  Gltf: Package,
  Texture: Image,
  Audio: Music,
  Video,
  EnvironmentMap: Mountain,
};

/** Colour by *family*, from the theme's own variables.
 *
 * Stroke icons arrive monochrome — they take `currentColor`, which is the whole
 * reason they theme at all. Emoji were multicoloured, but incidentally: whatever
 * the OS font decided, unrelated to the editor's palette and impossible to change.
 * Colouring here buys back the at-a-glance grouping while keeping it *meaningful*
 * — a light is yellow because it is a light, and a geometry primitive is blue
 * because it is geometry.
 *
 * Reuse across distant families is deliberate: nobody mistakes a texture for an
 * interaction zone, and nine theme colours will not stretch to twenty-eight kinds
 * without inventing colours the theme does not have.
 */
const KIND_COLOR: Record<string, string> = {
  Cube: "var(--blue)", Sphere: "var(--blue)", Cylinder: "var(--blue)",
  Capsule: "var(--blue)", Plane: "var(--blue)", Tetrahedron: "var(--blue)",

  DirectionalLight: "var(--yellow)", PointLight: "var(--yellow)",
  SpotLight: "var(--yellow)", AmbientLight: "var(--yellow)",

  Camera: "var(--blue-l)",

  Text: "var(--subtext1)", ExtrudedText: "var(--subtext1)",
  HudText: "var(--subtext1)", Billboard: "var(--subtext1)",

  Player: "var(--red)", PlayerSpawn: "var(--red)",
  PlayerSpawnZone: "var(--red)", PlayerAnchor: "var(--red)",

  Effect: "var(--peach)", EffectBurst: "var(--peach)", EffectTrail: "var(--peach)",

  Panel: "var(--mauve)", InteractionZone: "var(--mauve)",

  // Asset kinds. A GltfAsset node and an imported Gltf are one idea, so they match.
  GltfAsset: "var(--flamingo)", Gltf: "var(--flamingo)",
  Texture: "var(--mauve)",
  AudioClip: "var(--green)", Audio: "var(--green)",
  Video: "var(--teal)",
  EnvironmentMap: "var(--peach)",

  Empty: "var(--overlay0)",
};

export function KindIcon({
  kind,
  size = 13,
  className,
}: {
  kind: string;
  size?: number;
  className?: string;
}) {
  // An unmapped kind gets a neutral mark rather than nothing: a missing icon
  // should read as "no icon for this yet", not as an empty column that looks like
  // a rendering bug.
  const Icon = KIND_ICONS[kind] ?? CircleDashed;
  // `color`, not `stroke`: the icon is `stroke="currentColor"`, so setting the
  // text colour tints it — and a caller can still override by colouring an
  // ancestor, which is what keeps a disabled row's icon dimming with its row.
  return (
    <Icon
      size={size}
      className={className}
      style={{ color: KIND_COLOR[kind] ?? "var(--overlay0)" }}
      aria-hidden
    />
  );
}

/** Whether a kind has an icon of its own, for callers that want to fall back. */
export function hasKindIcon(kind: string): boolean {
  return kind in KIND_ICONS;
}
