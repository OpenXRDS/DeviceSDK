use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsGrabType, XrdsInteractionZoneShape, XrdsPlayerLocomotionMode,
    XrdsSceneAmbientLight, XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCube,
    XrdsSceneDirectionalLight, XrdsSceneDocument, XrdsSceneInteractionZone, XrdsSceneMaterial,
    XrdsSceneNode, XrdsSceneNodeId, XrdsSceneNodePayload, XrdsScenePlane3D,
    XrdsScenePlayerSpawn, XrdsSceneSphere, XrdsSceneTransform,
};

// ── Descriptor ────────────────────────────────────────────────────────────────

pub struct SceneTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub const ALL_TEMPLATES: &[SceneTemplate] = &[
    SceneTemplate {
        id: "empty",
        name: "Empty Scene",
        description: "A completely blank scene. Add everything from scratch.",
    },
    SceneTemplate {
        id: "simple_3d",
        name: "Simple 3D Scene",
        description: "Ground plane, ambient light, directional sun, and a cube.\nReady to explore with the orbit camera.",
    },
    SceneTemplate {
        id: "basic_interactive",
        name: "Basic Interactive",
        description: "Two objects with interaction zones (hover + grab).\nDemonstrates the interaction system.",
    },
    SceneTemplate {
        id: "vr_experience",
        name: "VR Experience",
        description: "Player spawn point, ground, lighting, and grabbable objects.\nPress Play to walk around. Set enable_xr: true to deploy to headset.",
    },
    SceneTemplate {
        id: "platformer",
        name: "Platformer",
        description: "Grounded locomotion with jump (Space).\nWASD to move, RMB + drag to look. Minimal kinematic controller — no physics engine required.",
    },
];

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub fn build_template(id: &str) -> XrdsSceneDocument {
    match id {
        "simple_3d" => build_simple_3d(),
        "basic_interactive" => build_basic_interactive(),
        "vr_experience" => build_vr_experience(),
        "platformer" => build_platformer(),
        _ => XrdsSceneDocument::default(),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn node(
    id: u64,
    parent_id: Option<u64>,
    name: &str,
    translation: [f32; 3],
    rotation_xyzw: [f32; 4],
    payload: XrdsSceneNodePayload,
) -> XrdsSceneNode {
    XrdsSceneNode {
        id: XrdsSceneNodeId(id),
        parent_id: parent_id.map(XrdsSceneNodeId),
        name: name.to_string(),
        enabled: true,
        visible: true,
        transform: XrdsSceneTransform {
            translation,
            rotation_quat_xyzw: rotation_xyzw,
            scale: [1.0, 1.0, 1.0],
        },
        payload,
        editor: XrdsEditorMetadata::default(),
    }
}

const IDENTITY_ROT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

// Quaternion: rotate -50° around X (sun angle — points down-and-forward)
// half-angle = -25°  →  xyzw = (sin(-25°), 0, 0, cos(-25°))
const SUN_ROT: [f32; 4] = [-0.4226, 0.0, 0.0, 0.9063];

// ── Template: Simple 3D Scene ─────────────────────────────────────────────────

fn build_simple_3d() -> XrdsSceneDocument {
    let mut doc = XrdsSceneDocument::default();
    doc.metadata.name = "Simple 3D Scene".to_string();

    doc.nodes.push(node(
        1, None, "Ambient Light",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::AmbientLight(XrdsSceneAmbientLight {
            color: [1.0, 0.95, 0.9, 1.0],
            brightness: 200.0,
            ..Default::default()
        }),
    ));

    doc.nodes.push(node(
        2, None, "Sun",
        [0.0, 5.0, -3.0], SUN_ROT,
        XrdsSceneNodePayload::DirectionalLight(XrdsSceneDirectionalLight {
            color: [1.0, 0.98, 0.92, 1.0],
            illuminance: 10_000.0,
            shadows: true,
        }),
    ));

    doc.nodes.push(node(
        3, None, "Ground",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
            size: [20.0, 20.0],
            material: XrdsSceneMaterial {
                base_color: [0.55, 0.55, 0.55, 1.0],
                ..Default::default()
            },
        }),
    ));

    doc.nodes.push(node(
        4, None, "Cube",
        [0.0, 0.5, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube::default()),
    ));

    doc.nodes.push(node(
        5, None, "Camera",
        [0.0, 3.0, 8.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Camera(XrdsSceneCamera {
            projection: XrdsSceneCameraProjection::Perspective {
                fov_deg: 60.0,
                near: 0.1,
                far: Some(1000.0),
                order: 0,
            },
            look_at: Some([0.0, 0.5, 0.0]),
        }),
    ));

    doc
}

// ── Template: Basic Interactive ───────────────────────────────────────────────

fn build_basic_interactive() -> XrdsSceneDocument {
    let mut doc = XrdsSceneDocument::default();
    doc.metadata.name = "Basic Interactive".to_string();

    doc.nodes.push(node(
        1, None, "Ambient Light",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::AmbientLight(XrdsSceneAmbientLight {
            color: [1.0, 0.95, 0.9, 1.0],
            brightness: 200.0,
            ..Default::default()
        }),
    ));

    doc.nodes.push(node(
        2, None, "Sun",
        [0.0, 5.0, -3.0], SUN_ROT,
        XrdsSceneNodePayload::DirectionalLight(XrdsSceneDirectionalLight {
            color: [1.0, 0.98, 0.92, 1.0],
            illuminance: 10_000.0,
            shadows: true,
        }),
    ));

    doc.nodes.push(node(
        3, None, "Ground",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
            size: [20.0, 20.0],
            material: XrdsSceneMaterial {
                base_color: [0.55, 0.55, 0.55, 1.0],
                ..Default::default()
            },
        }),
    ));

    // Grabbable cube (left)
    doc.nodes.push(node(
        4, None, "Grab Cube",
        [-1.5, 0.5, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube {
            material: XrdsSceneMaterial {
                base_color: [0.25, 0.5, 1.0, 1.0],
                ..Default::default()
            },
            ..Default::default()
        }),
    ));

    // Interaction zone on cube (free grab + hover)
    doc.nodes.push(node(
        5, Some(4), "Grab Zone",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::InteractionZone(XrdsSceneInteractionZone {
            shape: XrdsInteractionZoneShape::Box { half_extents: [0.5, 0.5, 0.5] },
            grab_type: XrdsGrabType::Free,
            hoverable: true,
        }),
    ));

    // Hoverable sphere (right)
    doc.nodes.push(node(
        6, None, "Hover Sphere",
        [1.5, 0.5, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
            material: XrdsSceneMaterial {
                base_color: [1.0, 0.4, 0.25, 1.0],
                ..Default::default()
            },
            ..Default::default()
        }),
    ));

    // Interaction zone on sphere (hover only)
    doc.nodes.push(node(
        7, Some(6), "Hover Zone",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::InteractionZone(XrdsSceneInteractionZone {
            shape: XrdsInteractionZoneShape::Sphere { radius: 0.5 },
            grab_type: XrdsGrabType::None,
            hoverable: true,
        }),
    ));

    doc.nodes.push(node(
        8, None, "Camera",
        [0.0, 3.0, 8.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Camera(XrdsSceneCamera {
            projection: XrdsSceneCameraProjection::Perspective {
                fov_deg: 60.0,
                near: 0.1,
                far: Some(1000.0),
                order: 0,
            },
            look_at: Some([0.0, 0.5, 0.0]),
        }),
    ));

    doc
}

// ── Template: VR Experience ───────────────────────────────────────────────────

fn build_vr_experience() -> XrdsSceneDocument {
    let mut doc = XrdsSceneDocument::default();
    doc.metadata.name = "VR Experience".to_string();

    // Lighting
    doc.nodes.push(node(
        1, None, "Ambient Light",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::AmbientLight(XrdsSceneAmbientLight {
            color: [0.9, 0.92, 1.0, 1.0],
            brightness: 150.0,
            ..Default::default()
        }),
    ));
    doc.nodes.push(node(
        2, None, "Sun",
        [0.0, 5.0, -3.0], SUN_ROT,
        XrdsSceneNodePayload::DirectionalLight(XrdsSceneDirectionalLight {
            color: [1.0, 0.97, 0.88, 1.0],
            illuminance: 15_000.0,
            shadows: true,
        }),
    ));

    // Ground — 10 m × 10 m walkable area
    doc.nodes.push(node(
        3, None, "Ground",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
            size: [10.0, 10.0],
            material: XrdsSceneMaterial {
                base_color: [0.45, 0.5, 0.45, 1.0],
                ..Default::default()
            },
        }),
    ));

    // Player spawn — floor level; runtime adds 1.6 m eye-height automatically
    doc.nodes.push(node(
        4, None, "Player Spawn",
        [0.0, 0.0, 4.0], IDENTITY_ROT,
        XrdsSceneNodePayload::PlayerSpawn(XrdsScenePlayerSpawn {
            locomotion_mode: XrdsPlayerLocomotionMode::Teleport,
            fov_deg: 90.0,
        }),
    ));

    // Pedestal
    doc.nodes.push(node(
        5, None, "Pedestal",
        [0.0, 0.5, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube {
            size: [0.4, 1.0, 0.4],
            material: XrdsSceneMaterial {
                base_color: [0.7, 0.7, 0.7, 1.0],
                ..Default::default()
            },
        }),
    ));

    // Grabbable sphere on the pedestal
    doc.nodes.push(node(
        6, None, "Grab Sphere",
        [0.0, 1.2, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
            radius: 0.18,
            material: XrdsSceneMaterial {
                base_color: [0.2, 0.55, 1.0, 1.0],
                ..Default::default()
            },
        }),
    ));
    doc.nodes.push(node(
        7, Some(6), "Grab Zone",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::InteractionZone(XrdsSceneInteractionZone {
            shape: XrdsInteractionZoneShape::Sphere { radius: 0.22 },
            grab_type: XrdsGrabType::Free,
            hoverable: true,
        }),
    ));

    // Hoverable cube to the left
    doc.nodes.push(node(
        8, None, "Hover Cube",
        [-1.5, 0.5, -1.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube {
            size: [0.6, 0.6, 0.6],
            material: XrdsSceneMaterial {
                base_color: [1.0, 0.4, 0.2, 1.0],
                ..Default::default()
            },
        }),
    ));
    doc.nodes.push(node(
        9, Some(8), "Hover Zone",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::InteractionZone(XrdsSceneInteractionZone {
            shape: XrdsInteractionZoneShape::Box { half_extents: [0.35, 0.35, 0.35] },
            grab_type: XrdsGrabType::None,
            hoverable: true,
        }),
    ));

    doc
}

// ── Template: Platformer ──────────────────────────────────────────────────────

fn build_platformer() -> XrdsSceneDocument {
    let mut doc = XrdsSceneDocument::default();
    doc.metadata.name = "Platformer".to_string();

    doc.nodes.push(node(
        1, None, "Ambient Light",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::AmbientLight(XrdsSceneAmbientLight {
            color: [0.85, 0.9, 1.0, 1.0],
            brightness: 180.0,
            ..Default::default()
        }),
    ));
    doc.nodes.push(node(
        2, None, "Sun",
        [0.0, 5.0, -3.0], SUN_ROT,
        XrdsSceneNodePayload::DirectionalLight(XrdsSceneDirectionalLight {
            color: [1.0, 0.96, 0.85, 1.0],
            illuminance: 12_000.0,
            shadows: true,
        }),
    ));

    doc.nodes.push(node(
        3, None, "Ground",
        [0.0, 0.0, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
            size: [20.0, 20.0],
            material: XrdsSceneMaterial {
                base_color: [0.4, 0.55, 0.4, 1.0],
                ..Default::default()
            },
        }),
    ));

    // Player spawn — Smooth mode; floor level, runtime adds 1.6 m eye-height
    doc.nodes.push(node(
        4, None, "Player Spawn",
        [0.0, 0.0, 5.0], IDENTITY_ROT,
        XrdsSceneNodePayload::PlayerSpawn(XrdsScenePlayerSpawn {
            locomotion_mode: XrdsPlayerLocomotionMode::Smooth,
            fov_deg: 80.0,
        }),
    ));

    // Low platform — top at y=0.5, one easy jump from ground
    doc.nodes.push(node(
        5, None, "Platform A",
        [0.0, 0.25, 1.5], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube {
            size: [3.0, 0.5, 2.0],
            material: XrdsSceneMaterial {
                base_color: [0.6, 0.45, 0.3, 1.0],
                ..Default::default()
            },
        }),
    ));

    // Mid platform — top at y=1.0
    doc.nodes.push(node(
        6, None, "Platform B",
        [3.5, 0.5, 0.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube {
            size: [2.0, 1.0, 2.0],
            material: XrdsSceneMaterial {
                base_color: [0.5, 0.4, 0.6, 1.0],
                ..Default::default()
            },
        }),
    ));

    // Obstacle columns
    doc.nodes.push(node(
        7, None, "Column L",
        [-2.0, 0.75, -1.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube {
            size: [0.5, 1.5, 0.5],
            material: XrdsSceneMaterial { base_color: [0.7, 0.7, 0.65, 1.0], ..Default::default() },
        }),
    ));
    doc.nodes.push(node(
        8, None, "Column R",
        [2.0, 0.75, -1.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Cube(XrdsSceneCube {
            size: [0.5, 1.5, 0.5],
            material: XrdsSceneMaterial { base_color: [0.7, 0.7, 0.65, 1.0], ..Default::default() },
        }),
    ));

    // Goal — bright yellow sphere at the far end
    doc.nodes.push(node(
        9, None, "Goal",
        [0.0, 0.3, -4.0], IDENTITY_ROT,
        XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
            radius: 0.3,
            material: XrdsSceneMaterial {
                base_color: [1.0, 0.85, 0.1, 1.0],
                ..Default::default()
            },
        }),
    ));

    doc
}
