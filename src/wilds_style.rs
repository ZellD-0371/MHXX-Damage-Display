//! Monster Hunter Wilds critical/reverse-critical accents for the MHXX JP 1.5.1 port.
//! Number glyphs are deliberately rendered by the original Forge/TTF path in lib.rs.
//! This module uses only the red/blue effect sprites from the embedded RGBA atlas.

use core::ptr;
use forge::{
    imgui::DrawList,
    log::prelude::{info, warn},
    sys::imgui::{
        ImTextureData, ImTextureFormat, ImVec2,
        ImTextureData_ImTextureData, ImTextureData_Create, ImTextureData_destroy,
        ImTextureData_GetPixels, ImTextureData_GetTexRef, igRegisterUserTexture,
    },
};

const ATLAS_WIDTH: i32 = 512;
const ATLAS_HEIGHT: i32 = 256;
const ATLAS_BYTES: usize = ATLAS_WIDTH as usize * ATLAS_HEIGHT as usize * 4;
static EMBEDDED_ATLAS: &[u8; ATLAS_BYTES] = include_bytes!("../assets/wilds-effects.rgba");

// Bounds of the red/pink (positive affinity) and blue (negative affinity) marks.
// Coordinates are inclusive; UV's upper edge is exclusive to avoid clipping.
#[derive(Clone, Copy)]
struct Rect { x0: u16, y0: u16, x1: u16, y1: u16 }
impl Rect {
    fn uv(self) -> (ImVec2, ImVec2) {
        (
            ImVec2::new(self.x0 as f32 / ATLAS_WIDTH as f32, self.y0 as f32 / ATLAS_HEIGHT as f32),
            ImVec2::new((self.x1 + 1) as f32 / ATLAS_WIDTH as f32, (self.y1 + 1) as f32 / ATLAS_HEIGHT as f32),
        )
    }
    fn aspect(self) -> f32 {
        (self.x1 - self.x0 + 1) as f32 / (self.y1 - self.y0 + 1) as f32
    }
}

const RED_MARK: Rect = Rect { x0: 0, y0: 139, x1: 87, y1: 164 };
const BLUE_MARK: Rect = Rect { x0: 96, y0: 139, x1: 183, y1: 164 };

static mut EFFECT_TEXTURE: *mut ImTextureData = ptr::null_mut();
static mut INITIALIZATION_ATTEMPTED: bool = false;

fn texture() -> *mut ImTextureData {
    // This function is called from the live foreground draw callback. Forge's ImGui
    // context/renderer should therefore already be initialized.
    unsafe {
        if !EFFECT_TEXTURE.is_null() {
            return EFFECT_TEXTURE;
        }
        if INITIALIZATION_ATTEMPTED {
            return ptr::null_mut();
        }
        INITIALIZATION_ATTEMPTED = true;

        if EMBEDDED_ATLAS.len() != ATLAS_BYTES {
            warn!("Wilds critical accents: invalid embedded RGBA byte count {}", EMBEDDED_ATLAS.len());
            return ptr::null_mut();
        }
        let tex = ImTextureData_ImTextureData();
        if tex.is_null() {
            warn!("Wilds critical accents: ImTextureData allocation failed");
            return ptr::null_mut();
        }
        ImTextureData_Create(tex, ImTextureFormat::RGBA32, ATLAS_WIDTH, ATLAS_HEIGHT);
        let pixels = ImTextureData_GetPixels(tex);
        if pixels.is_null() {
            ImTextureData_destroy(tex);
            warn!("Wilds critical accents: pixel allocation failed");
            return ptr::null_mut();
        }
        ptr::copy_nonoverlapping(EMBEDDED_ATLAS.as_ptr(), pixels.cast::<u8>(), ATLAS_BYTES);
        igRegisterUserTexture(tex);
        EFFECT_TEXTURE = tex;
        info!("MHXX Wilds critical/reverse-critical star atlas registered (512x256 RGBA)");
        tex
    }
}

/// Draw the affinity accent around existing TTF/Forge text.
/// critical_state: 0 = no affinity trigger, 1 = reverse critical, 2 = critical.
pub fn draw_effects(
    draw: &DrawList,
    x: f32,
    y: f32,
    text_width: f32,
    size: f32,
    critical_state: u8,
    alpha: u8,
) {
    if !(critical_state == 1 || critical_state == 2) || text_width <= 0.0 || size <= 0.0 {
        return;
    }
    let texture = texture();
    if texture.is_null() {
        return;
    }
    let reverse = critical_state == 1;
    let rect = if reverse { BLUE_MARK } else { RED_MARK };
    let (uv0, uv1) = rect.uv();
    let line_rgb = if reverse { [48, 150, 255] } else { [255, 54, 82] };
    let line_alpha = ((u16::from(alpha) * 220) / 255) as u8;
    let line_color = forge::imgui::color(line_rgb[0], line_rgb[1], line_rgb[2], line_alpha);

    // A thin line brackets the TTF digits. The matching sprite is layered over its
    // center, so the atlas contributes the Wilds star effect but never the digits.
    let pad = (size * 0.08).max(1.0);
    let x0 = x - pad;
    let x1 = x + text_width + pad;
    let line_y = if reverse { y + size + size * 0.10 } else { y - size * 0.10 };
    draw.add_line(
        ImVec2::new(x0, line_y),
        ImVec2::new(x1, line_y),
        line_color,
        (size * 0.035).max(1.0),
    );

    let mark_h = (size * 0.32).max(7.0);
    let mark_w = mark_h * rect.aspect();
    let center_x = (x0 + x1) * 0.5;
    let center_y = line_y;
    let mark_x = center_x - mark_w * 0.5;
    let mark_y = center_y - mark_h * 0.5;
    let mark_alpha = ((u16::from(alpha) * 250) / 255) as u8;
    let tint = forge::imgui::color(255, 255, 255, mark_alpha);
    let tex_ref = unsafe { ImTextureData_GetTexRef(texture) };
    draw.add_image(
        tex_ref,
        ImVec2::new(mark_x, mark_y),
        ImVec2::new(mark_x + mark_w, mark_y + mark_h),
        uv0,
        uv1,
        tint,
    );
}
