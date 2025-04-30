use glam::{vec3, Vec3};

#[derive(Debug, Clone, Copy)]
pub enum LightType {
    Directional,
    Point(PointLightDescription),
    Spot(SpotLightDescription),
}

#[derive(Debug, Clone, Copy)]
pub struct PointLightDescription {
    pub range: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SpotLightDescription {
    pub range: f32,
    pub inner_cons_cos: f32,
    pub outer_cons_cos: f32,
}

#[derive(Debug, Clone)]
pub struct LightDescription {
    pub color: glam::Vec3,
    pub intensity: f32,
    pub ty: LightType,
    pub cast_shadow: bool,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct XrdsLight {
    pub view: glam::Mat4,
    pub view_proj: glam::Mat4,
    pub direction: glam::Vec3,
    pub range: f32,
    pub light_color: glam::Vec3,
    pub intensity: f32,
    pub position: glam::Vec3,
    pub light_type: u32,
    pub inner_cons_cos: f32,
    pub outer_cons_cos: f32,
    pub cast_shadow: u32,
    pub shadow_map_index: u32,
    _pad: [f32; 16],
}

pub struct LightColor;

impl LightColor {
    pub const CANDLE: Vec3 = vec3(1.0, 0.5764706, 0.16078432);
    pub const TUNGSTEN_40W: Vec3 = vec3(1.0, 0.77254903, 0.56078434);
    pub const TUNGSTEN_100W: Vec3 = vec3(1.0, 0.8392157, 0.6666667);
    pub const HALOGENE: Vec3 = vec3(1.0, 0.94509804, 0.8784314);
    pub const CARBON_ARC: Vec3 = vec3(1.0, 0.98039216, 0.95686275);
    pub const HIGH_NOON_SUN: Vec3 = vec3(1.0, 1.0, 0.9843137);
    pub const DIRECT_SUNLIGHT: Vec3 = vec3(1.0, 1.0, 1.0);
    pub const OVERCAST_SKY: Vec3 = vec3(0.7882353, 0.8862745, 1.0);
    pub const CLEAR_BLUE_SKY: Vec3 = vec3(0.2509804, 0.6117647, 1.0);
    pub const WARM_FLUORESCENT: Vec3 = vec3(1.0, 0.95686275, 0.8980392);
    pub const STANDARD_FLUORESCENT: Vec3 = vec3(0.95686275, 1.0, 0.98039216);
    pub const COOL_WHITE_FLUORESCENT: Vec3 = vec3(0.83137256, 0.92156863, 1.0);
    pub const FULL_SPECTRUM_FLUORESCENT: Vec3 = vec3(1.0, 0.95686275, 0.9490196);
    pub const GROW_LIGHT_FLUORESCENT: Vec3 = vec3(1.0, 0.9372549, 0.96862745);
    pub const BLACK_LIGHT_FLUORESCENT: Vec3 = vec3(0.654902, 0.0, 1.0);
    pub const MERCURY_VAPOR: Vec3 = vec3(0.84705883, 0.96862745, 1.0);
    pub const SODIUM_VAPOR: Vec3 = vec3(1.0, 0.81960785, 0.69803923);
    pub const METAL_HALIDE: Vec3 = vec3(0.9490196, 0.9882353, 1.0);
    pub const HIGH_PRESSURE_SODIUM: Vec3 = vec3(1.0, 0.7176471, 0.29803923);
}

impl LightType {
    pub fn shadowmap_count(&self) -> usize {
        match *self {
            LightType::Point(_) => 6,
            _ => 1,
        }
    }

    pub fn range(&self) -> f32 {
        match *self {
            LightType::Directional => f32::MAX,
            LightType::Point(description) => description.range,
            LightType::Spot(description) => description.range,
        }
    }
}

// impl XrdsLight {
//     pub fn new(light_description: LightDescription) -> Self {
//         let mut light = XrdsLight::default();
//         light.light_color = light_description.color;
//         light.intensity = light_description.intensity;
//         light.cast_shadow = light_description.cast_shadow.then(|| 1).unwrap_or(0);
//         light.shadow_map_index = std::u32::MAX; // initial

//         match light_description.ty {
//             LightType::Directional(description) => {
//                 light.light_type = LIGHT_TYPE_DIRECTIONAL;
//                 light.direction = description.direction;
//             }
//             LightType::Point(description) => {
//                 light.light_type = LIGHT_TYPE_POINT;
//                 light.position = description.position;
//                 light.range = description.range;
//             }
//             LightType::Spot(description) => {
//                 light.light_type = LIGHT_TYPE_SPOT;
//                 light.position = description.position;
//                 light.direction = description.direction;
//                 light.inner_cons_cos = description.inner_cons_cos;
//                 light.outer_cons_cos = description.outer_cons_cos;
//                 light.range = description.range;
//             }
//         };

//         light
//     }

//     pub fn encode(&self, render_pass: &RenderPass<'_>, transform: &Transform) {}
// }
