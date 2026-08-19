use crate::{XrdsColor, XrdsLinearRgba};

#[derive(Debug, Clone, Copy)]
pub struct CubeGeometryParams {
    pub size: [f32; 3],
}

impl Default for CubeGeometryParams {
    fn default() -> Self {
        Self {
            size: [1.0, 1.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CylinderGeometryParams {
    pub radius: f32,
    pub height: f32,
}

impl Default for CylinderGeometryParams {
    fn default() -> Self {
        Self {
            radius: 0.5,
            height: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CapsuleGeometryParams {
    pub radius: f32,
    /// Excludes the two hemispherical caps — see `XrdsCapsule::length`.
    pub length: f32,
}

impl Default for CapsuleGeometryParams {
    fn default() -> Self {
        Self {
            radius: 0.5,
            length: 1.0,
        }
    }
}

/// Tunable parameters of an `XrdsEffect`, as passed to
/// `XrdsAPI::set_effect_params`.
///
/// Mirrors the descriptor's tunable fields but omits identity/placement (`name`,
/// `transform`, `visible`), which are handled by the common updaters every
/// surface type already gets. `kind` is included: switching Burst <-> Trail is a
/// parameter change, not a different primitive.
#[derive(Debug, Clone, Copy)]
pub struct EffectParams {
    pub kind: crate::primitives::XrdsEffectKind,
    /// Whether the effect emits as soon as it exists. `false` leaves it idle,
    /// waiting for a trigger — see `XrdsEffect::auto_play`.
    pub auto_play: bool,
    /// Total particles per firing; used only when `kind` is `Burst`.
    pub burst_count: u32,
    /// Particles per second; used only when `kind` is `Trail`.
    pub spawn_rate: f32,
    pub lifetime_secs: f32,
    pub size_min: f32,
    pub size_max: f32,
    /// Keep components <= 1.0 — see `XrdsEffect::color_start` for why. Values
    /// above 1.0 are clamped when the effect is built.
    pub color_start: crate::XrdsColor,
    pub color_end: crate::XrdsColor,
    pub speed_min: f32,
    pub speed_max: f32,
    /// When true, emit outward in every direction and ignore `spread_deg`.
    pub omnidirectional: bool,
    /// Cone half-angle about local +Y in degrees; ignored if `omnidirectional`.
    pub spread_deg: f32,
    pub gravity: [f32; 3],
    pub emission_radius: f32,
    pub blend: crate::primitives::XrdsEffectBlend,
    /// End-of-life size multiplier; `1.0` holds size constant.
    pub size_end: f32,
    pub drag: f32,
    pub fade_edge: f32,
    pub fade_scene: f32,
}

impl Default for EffectParams {
    fn default() -> Self {
        let effect = crate::primitives::XrdsEffect::new();
        Self {
            kind: effect.kind,
            auto_play: effect.auto_play,
            burst_count: effect.burst_count,
            spawn_rate: effect.spawn_rate,
            lifetime_secs: effect.lifetime_secs,
            size_min: effect.size_min,
            size_max: effect.size_max,
            color_start: effect.color_start,
            color_end: effect.color_end,
            speed_min: effect.speed_min,
            speed_max: effect.speed_max,
            omnidirectional: effect.omnidirectional,
            spread_deg: effect.spread_deg,
            gravity: effect.gravity,
            emission_radius: effect.emission_radius,
            blend: effect.blend,
            size_end: effect.size_end,
            drag: effect.drag,
            fade_edge: effect.fade_edge,
            fade_scene: effect.fade_scene,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SphereGeometryParams {
    pub radius: f32,
}

impl Default for SphereGeometryParams {
    fn default() -> Self {
        Self { radius: 0.5 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Plane3DGeometryParams {
    pub size: [f32; 2],
}

impl Default for Plane3DGeometryParams {
    fn default() -> Self {
        Self { size: [1.0, 1.0] }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TetrahedronGeometryParams {
    pub vertices: [[f32; 3]; 4],
}

impl Default for TetrahedronGeometryParams {
    fn default() -> Self {
        Self {
            vertices: [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsMaterialParams {
    pub base_color: XrdsColor,
    pub emissive: XrdsLinearRgba,
    pub opacity: f32,
    pub unlit: bool,
    pub pbr: XrdsMaterialPbrParams,
    pub textures: XrdsMaterialTextureSlots,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsMaterialTextureRef {
    pub texture_asset_id: String,
    pub uv: XrdsMaterialTextureUvParams,
    pub sampler: XrdsMaterialTextureSamplerParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XrdsMaterialTextureSlotKind {
    BaseColor,
    MetallicRoughness,
    Normal,
    Occlusion,
    Emissive,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct XrdsMaterialTextureSlots {
    pub base_color: Option<XrdsMaterialTextureRef>,
    pub metallic_roughness: Option<XrdsMaterialTextureRef>,
    pub normal: Option<XrdsMaterialTextureRef>,
    pub occlusion: Option<XrdsMaterialTextureRef>,
    pub emissive: Option<XrdsMaterialTextureRef>,
}

impl XrdsMaterialTextureSlots {
    pub fn is_empty(&self) -> bool {
        self.base_color.is_none()
            && self.metallic_roughness.is_none()
            && self.normal.is_none()
            && self.occlusion.is_none()
            && self.emissive.is_none()
    }

    pub fn get(&self, slot: XrdsMaterialTextureSlotKind) -> Option<&XrdsMaterialTextureRef> {
        match slot {
            XrdsMaterialTextureSlotKind::BaseColor => self.base_color.as_ref(),
            XrdsMaterialTextureSlotKind::MetallicRoughness => self.metallic_roughness.as_ref(),
            XrdsMaterialTextureSlotKind::Normal => self.normal.as_ref(),
            XrdsMaterialTextureSlotKind::Occlusion => self.occlusion.as_ref(),
            XrdsMaterialTextureSlotKind::Emissive => self.emissive.as_ref(),
        }
    }

    pub fn set(
        &mut self,
        slot: XrdsMaterialTextureSlotKind,
        texture: Option<XrdsMaterialTextureRef>,
    ) {
        match slot {
            XrdsMaterialTextureSlotKind::BaseColor => self.base_color = texture,
            XrdsMaterialTextureSlotKind::MetallicRoughness => self.metallic_roughness = texture,
            XrdsMaterialTextureSlotKind::Normal => self.normal = texture,
            XrdsMaterialTextureSlotKind::Occlusion => self.occlusion = texture,
            XrdsMaterialTextureSlotKind::Emissive => self.emissive = texture,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsMaterialTextureUvParams {
    pub set: u32,
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub rotation_deg: f32,
    pub transform_mode: XrdsMaterialTextureUvTransformMode,
}

impl Default for XrdsMaterialTextureUvParams {
    fn default() -> Self {
        Self {
            set: 0,
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation_deg: 0.0,
            transform_mode: XrdsMaterialTextureUvTransformMode::Centered,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsMaterialTextureUvTransformMode {
    Centered,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XrdsMaterialTextureSamplerParams {
    pub wrap_u: XrdsMaterialTextureWrapMode,
    pub wrap_v: XrdsMaterialTextureWrapMode,
    pub min_filter: XrdsMaterialTextureFilterMode,
    pub mag_filter: XrdsMaterialTextureFilterMode,
    pub mipmap_filter: XrdsMaterialTextureFilterMode,
}

impl Default for XrdsMaterialTextureSamplerParams {
    fn default() -> Self {
        Self {
            wrap_u: XrdsMaterialTextureWrapMode::Repeat,
            wrap_v: XrdsMaterialTextureWrapMode::Repeat,
            min_filter: XrdsMaterialTextureFilterMode::Linear,
            mag_filter: XrdsMaterialTextureFilterMode::Linear,
            mipmap_filter: XrdsMaterialTextureFilterMode::Linear,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsMaterialTextureWrapMode {
    Repeat,
    MirroredRepeat,
    ClampToEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsMaterialTextureFilterMode {
    Linear,
    Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsMaterialAlphaMode {
    Auto,
    Opaque,
    Mask,
    Blend,
}

impl Default for XrdsMaterialAlphaMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsMaterialPbrParams {
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,
    pub double_sided: bool,
    pub alpha_mode: XrdsMaterialAlphaMode,
    pub alpha_cutoff: f32,
}

impl Default for XrdsMaterialPbrParams {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            roughness: 0.5,
            reflectance: 0.5,
            double_sided: false,
            alpha_mode: XrdsMaterialAlphaMode::Auto,
            alpha_cutoff: 0.5,
        }
    }
}

impl Default for XrdsMaterialParams {
    fn default() -> Self {
        Self {
            base_color: XrdsColor::WHITE,
            emissive: XrdsLinearRgba::BLACK,
            opacity: 1.0,
            unlit: false,
            pbr: XrdsMaterialPbrParams::default(),
            textures: XrdsMaterialTextureSlots::default(),
        }
    }
}

/// Transform parameters for XRDS scene objects.
///
/// Rotation is a quaternion, and only a quaternion. To author in degrees use
/// [`TransformParams::with_euler_deg`] or [`TransformParams::set_euler_deg`]; to
/// display degrees use [`TransformParams::euler_deg`].
///
/// # Why there is no `rotation_euler_xyz_deg` field
///
/// There was one until 2026-08-19, alongside the quaternion, with the quaternion
/// documented as authoritative and the euler field described as "an authoring
/// convenience, kept in sync during export, never read by the runtime". Two ways to
/// say one thing, with a precedence rule you had to read the docs to learn.
///
/// It behaved exactly as that arrangement predicts. Every non-example site in the
/// tree only ever *wrote* it, to keep it in sync; nothing read it. And
/// `examples/xrds_first/parent_child.rs` set **only** the euler field for its
/// grandchild plane — a plain `[-90, 0, 0]` that silently did nothing, in a shipped
/// example, because the runtime reads the quaternion. Nobody noticed, because
/// there is no way to notice: it compiles, it round-trips, it renders unrotated.
///
/// A field that can be set and ignored is the failure this SDK keeps paying for, so
/// the field is gone and degrees are now a conversion rather than a second source of
/// truth. The document format is unaffected — `XrdsSceneTransform` has always
/// serialized the quaternion alone.
#[derive(Debug, Clone, Copy)]
pub struct TransformParams {
    pub translation: [f32; 3],
    pub rotation_quat_xyzw: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for TransformParams {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl TransformParams {
    /// Set the rotation from XYZ Euler angles in degrees, consuming `self`.
    ///
    /// Applied in the same order as [`glam::Quat::from_euler`] with
    /// [`glam::EulerRot::XYZ`], which is what the runtime's own conversions use.
    pub fn with_euler_deg(mut self, x_deg: f32, y_deg: f32, z_deg: f32) -> Self {
        self.set_euler_deg(x_deg, y_deg, z_deg);
        self
    }

    /// Set the rotation from XYZ Euler angles in degrees.
    pub fn set_euler_deg(&mut self, x_deg: f32, y_deg: f32, z_deg: f32) {
        let q = glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            x_deg.to_radians(),
            y_deg.to_radians(),
            z_deg.to_radians(),
        );
        self.rotation_quat_xyzw = [q.x, q.y, q.z, q.w];
    }

    /// This rotation as XYZ Euler angles in degrees.
    ///
    /// For display and authoring. Note that a quaternion has more than one Euler
    /// representation, so this is not guaranteed to return the same triple that was
    /// passed to [`Self::set_euler_deg`] — only an equivalent one. That ambiguity is
    /// inherent to Euler angles and is the other reason not to store them.
    pub fn euler_deg(&self) -> [f32; 3] {
        let [x, y, z, w] = self.rotation_quat_xyzw;
        let (rx, ry, rz) = glam::Quat::from_xyzw(x, y, z, w).to_euler(glam::EulerRot::XYZ);
        [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()]
    }
}

#[cfg(test)]
mod transform_rotation_tests {
    use crate::TransformParams;

    /// The bug the dual field caused, now impossible to reproduce: setting degrees
    /// must change the rotation the runtime reads.
    #[test]
    fn setting_degrees_changes_the_quaternion_the_runtime_reads() {
        let identity = TransformParams::default().rotation_quat_xyzw;
        let rotated = TransformParams::default().with_euler_deg(-90.0, 0.0, 0.0);
        assert_ne!(
            rotated.rotation_quat_xyzw, identity,
            "-90 degrees about X left the quaternion at identity",
        );
    }

    #[test]
    fn degrees_round_trip_to_an_equivalent_rotation() {
        for angles in [
            [-90.0_f32, 0.0, 0.0],
            [0.0, 45.0, 0.0],
            [0.0, 0.0, 90.0],
            [30.0, -20.0, 10.0],
        ] {
            let t = TransformParams::default().with_euler_deg(angles[0], angles[1], angles[2]);
            let back = TransformParams::default().with_euler_deg(
                t.euler_deg()[0],
                t.euler_deg()[1],
                t.euler_deg()[2],
            );
            // Compared as quaternions, not as angle triples: a rotation has more
            // than one Euler representation, so `euler_deg` may return a different
            // — but equivalent — triple. `q` and `-q` are also the same rotation.
            let a = t.rotation_quat_xyzw;
            let b = back.rotation_quat_xyzw;
            let dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
            assert!(
                (dot.abs() - 1.0).abs() < 1e-4,
                "{angles:?} did not round-trip: {a:?} vs {b:?}",
            );
        }
    }

    #[test]
    fn default_rotation_is_identity_in_both_representations() {
        let t = TransformParams::default();
        assert_eq!(t.rotation_quat_xyzw, [0.0, 0.0, 0.0, 1.0]);
        for angle in t.euler_deg() {
            assert!(angle.abs() < 1e-5, "default euler was {:?}", t.euler_deg());
        }
    }
}

