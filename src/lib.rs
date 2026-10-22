//! MHXX Japan 1.5.1 experimental port: hit capture -> damage -> projection -> Forge drawing.
#![no_std]

mod data;
mod wilds_style; // WILDS_STYLE_INTEGRATED
#[path = "../shared/plugin_config.rs"]
mod plugin_config;

use core::{
    arch::asm,
    ffi::c_void,
    mem::{self, MaybeUninit},
    ptr,
    sync::atomic::Ordering,
};
use data::*;
use forge::{
    imgui,
    log::prelude::{error, info, warn},
    sys::imgui::{ImGuiInputTextFlags, ImVec2},
};

// Forge 入口、更新和绘图。
// 字体仅在 ImGui 初始化时写入，随后由同一渲染线程读取。
const DIAGNOSTIC_ONLY: bool = false;
static JP_DIAGNOSTIC_FRAME: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static mut CUSTOM_FONT: Option<imgui::ImFont> = None;
static JP_COLLISION_PROBE_CALLS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static JP_DAMAGE_PROBE_CALLS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static MENU: Shared<plugin_config::Edit<Config>> =
    Shared::new(plugin_config::Edit::new(Config::DEFAULT));
static FONT_INPUT: Shared<[u8; 168]> = Shared::new([0; 168]);

#[forge::entry]
fn main() {
    let base = forge::mem::text_addr();
    GAME_BASE.store(base, Ordering::Relaxed);
    if compatible(base) {
        if DIAGNOSTIC_ONLY {
            unsafe { diagnose_globals(base) };
            unsafe { install_collision_probe(base) };
            unsafe { install_damage_probe(base) };
            return;
        }
        let config = load_config();
        MENU.with(|edit| *edit = plugin_config::Edit::new(config));
        set_font_input(config);
        STATE.with(|state| state.config = config);
        unsafe { install_hooks(base) };
        INSTALLED.store(true, Ordering::Release);
        ENABLED.store(config.enabled, Ordering::Relaxed);
        info!("mhxx-jp-1.5.1-attack-damage-display v0.5: local attack damage, main=0x{base:08X}");
    }
}

fn load_config() -> Config {
    let mut bytes = [0; CONFIG_MAX_BYTES];
    match plugin_config::DAMAGE.read(&mut bytes) {
        Ok(text) => match Config::parse(text) {
            Ok(config) => return config,
            Err(error) => warn!("damage save INI line {}: {}", error.line, error.message),
        },
        Err(error) => warn!("damage save INI: {error:?}; using defaults"),
    }
    Config::DEFAULT
}

fn file_exists(path: &core::ffi::CStr) -> bool {
    // 直接传入已有 C 字符串，避免 SDK 的 CString 包装引入 malloc/free 导入。
    unsafe { forge::sys::fs::forge_fs_fileExists(path.as_ptr()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn forge_onImGuiInit(
    _context: *mut c_void,
    _alloc: *mut c_void,
    _free: *mut c_void,
) {
    let config = STATE.with(|state| state.config);
    if let Some(file) = config.font_file {
        let path = file.path();
        let font = if file_exists(path) {
            imgui::ImFont::load(path)
        } else {
            None
        };
        match font {
            Some(font) => {
                unsafe { CUSTOM_FONT = Some(font) };
                info!(
                    "mhxx-jp-1.5.1-attack-damage-display: loaded font {}",
                    path.to_str().unwrap()
                );
            }
            None => warn!(
                "mhxx-jp-1.5.1-attack-damage-display: could not load font {}; using Forge font",
                path.to_str().unwrap()
            ),
        }
    }
    // Wilds atlas initialization is deferred until the first live draw callback.
}

#[unsafe(no_mangle)]
pub extern "C" fn forge_onUpdate(_dt: f32) {
    if DIAGNOSTIC_ONLY {
        let base = GAME_BASE.load(Ordering::Relaxed);
        if base != 0 {
            unsafe { diagnose_runtime_getter(base) };
        }
        return;
    }
    // 在游戏更新结束后投影；NVN 绘制回调只读插件快照，不访问游戏对象。
    let mut snapshot = STATE.with(|state| state.snapshot());
    if !snapshot.any_active() {
        return;
    }
    let base = GAME_BASE.load(Ordering::Relaxed);
    let player = unsafe { local_player(base) };
    if player != snapshot.epoch.player || unsafe { epoch(player) } != snapshot.epoch {
        STATE.with(|state| state.clear_epoch(snapshot.epoch));
        return;
    }
    let projection = unsafe { Projection::current(base) };
    snapshot.display = unsafe { display_area(base) };
    for event in snapshot.events.iter_mut().filter(|event| event.active) {
        event.visible = false;
        if let Some(screen) = unsafe { projection.project(event.world) } {
            event.screen = screen;
            event.visible = true;
        }
    }
    STATE.with(|state| state.commit_projection(&snapshot));
}

#[forge::imgui_free_render]
fn draw_damage() {
    let seconds = imgui::get_io().DeltaTime;
    let seconds = if seconds.is_finite() && seconds > 0.0 {
        seconds
    } else {
        1.0 / 60.0
    };
    let snapshot = STATE.with(|state| state.advance(seconds));
    let config = snapshot.config;
    let draw = imgui::DrawList::foreground();
    let font = unsafe { CUSTOM_FONT }.unwrap_or_else(imgui::ImFont::current);
    // Forge 的 DisplaySize 是整张交换纹理；掌机画面只使用其中的 1280×720。
    let scale = snapshot.display.scale();
    imgui::push_font(&font, Some(config.font_size * scale));
    let mut visible = 0;
    for event in snapshot
        .events
        .iter()
        .filter(|event| event.active && event.visible)
    {
        let size = (config.font_size + 4.0 * (1.0 - event.age / 0.15).max(0.0)) * scale;
        let text = event.digits.as_str();
        let text_size = imgui::calc_text_size(text, false, 0.0);
        let width = text_size.x * size / imgui::ImFont::current_size();
        let position = snapshot.display.position(event.screen);
        let x = position[0] - width * 0.5 + ((event.serial % 5) as f32 - 2.0) * 6.0 * scale;
        let y = position[1]
            - size
            - ((event.age / config.full_opacity).min(1.0) * 30.0 + (event.serial % 3) as f32 * 5.0)
                * scale;
        let alpha = event.alpha(&config);
        let outline = imgui::color(
            config.outline_color[0],
            config.outline_color[1],
            config.outline_color[2],
            (u16::from(alpha) * u16::from(config.outline_color[3]) / 255) as u8,
        );
        let edge = config.outline_width * scale;
        let rgb = if event.yellow {
            config.highlight_color
        } else {
            config.normal_color
        };
        let color = imgui::color(
            rgb[0],
            rgb[1],
            rgb[2],
            (u16::from(alpha) * u16::from(rgb[3]) / 255) as u8,
        );
        // Prefer the Wilds number atlas, but retain the proven font renderer as a fallback.
        if true /* keep Forge TTF digits; Wilds accents only */ {
            if edge > 0.0 {
                for (dx, dy) in [
                    (-edge, 0.0),
                    (edge, 0.0),
                    (0.0, -edge),
                    (0.0, edge),
                    (-edge, -edge),
                    (-edge, edge),
                    (edge, -edge),
                    (edge, edge),
                ] {
                    draw.add_text_with_font(&font, size, ImVec2::new(x + dx, y + dy), outline, text);
                }
            }
            draw.add_text_with_font(&font, size, ImVec2::new(x, y), color, text);
            wilds_style::draw_effects(&draw, x, y, width, size, event.critical_state, alpha);
        }
        visible += 1;
    }
    imgui::pop_font();
    VISIBLE.store(visible, Ordering::Relaxed);
}

#[forge::imgui_render]
fn draw_settings() {
    // Forge 共用一个 Plugin UI 窗口；折叠标题不会替内部控件建立 ID 作用域。
    imgui::push_id(imgui::Id::Str(c"attack_damage_display"));
    if imgui::tree_node(c"Attack Damage Display") {
        draw_settings_body();
        imgui::tree_pop();
    }
    imgui::pop_id();
}

fn draw_settings_body() {
    if DIAGNOSTIC_ONLY {
        imgui::text("Japanese MHXX 1.5.1 diagnostic mode: hooks intentionally disabled.");
        imgui::text("Inspect Forge logs for the raw manager-slot values.");
        return;
    }
    if !INSTALLED.load(Ordering::Acquire) {
        imgui::text("Hook addresses did not match. See Forge log.");
        return;
    }
    MENU.with(|edit| {
        let c = &mut edit.draft;
        imgui::checkbox(c"Show attack damage", &mut c.enabled);
        plugin_config::number(c"Font size", &mut c.font_size, 8.0, 128.0);
        plugin_config::number(c"Outline width", &mut c.outline_width, 0.0, 10.0);
        plugin_config::number(c"Full opacity (seconds)", &mut c.full_opacity, 0.01, 59.99);
        plugin_config::number(c"Total lifetime (seconds)", &mut c.lifetime, 0.02, 60.0);
        plugin_config::color4(c"Hitzone 45+ / fixed damage color", &mut c.highlight_color);
        plugin_config::color4(c"Normal damage color", &mut c.normal_color);
        plugin_config::color4(c"Outline color", &mut c.outline_color);
        let mut input = FONT_INPUT.with(|b| *b);
        if imgui::input_text(c"Font TTF filename", &mut input, ImGuiInputTextFlags::None) {
            FONT_INPUT.with(|b| *b = input);
            edit.message = "";
            if let Ok(name) = plugin_config::input_text(&input) {
                if name == "default" {
                    c.font_file = None;
                } else if let Some(file) = FontFile::new(name) {
                    c.font_file = Some(file);
                }
            }
        }
        imgui::text("Appearance previews live. Font file changes need save and restart.");
        let valid_timing = c.full_opacity > 0.0 && c.full_opacity < c.lifetime;
        if !valid_timing {
            imgui::text("Keep 0 < full opacity < total lifetime <= 60.");
        }
        let reset = plugin_config::footer(
            edit,
            Config::DEFAULT,
            &plugin_config::DAMAGE,
            false,
            |config| {
                let name = plugin_config::input_text(&input)?;
                if name != "default" && FontFile::new(name).is_none() {
                    return Err("Font must be default or a .ttf filename without a directory.");
                }
                let mut text = plugin_config::Text::<CONFIG_MAX_BYTES>::new();
                config
                    .write_ini(&mut text)
                    .map_err(|_| "Configuration too large.")?;
                Config::parse(text.as_str()).map_err(|e| e.message)?;
                plugin_config::DAMAGE.save(text.as_str())
            },
        );
        if reset {
            set_font_input(edit.draft);
        }
        let config = edit.draft;
        if config.full_opacity > 0.0 && config.full_opacity < config.lifetime {
            let changed = ENABLED.swap(config.enabled, Ordering::Relaxed) != config.enabled;
            STATE.with(|state| {
                if changed {
                    state.clear();
                }
                state.config = config;
            });
        }
    });
    imgui::text("Numbers appear at the hit point");
    imgui::text("Hitzone >=45 / fixed: highlight color; <45: normal color");
    for (label, value) in [
        ("Local contacts:", TOTAL_HITS.load(Ordering::Relaxed)),
        ("Damage events:", TOTAL_EVENTS.load(Ordering::Relaxed)),
        ("Visible numbers:", VISIBLE.load(Ordering::Relaxed)),
    ] {
        imgui::text(label);
        imgui::same_line(0.0, -1.0);
        imgui::text(Digits::new(value).as_str());
    }
}

fn set_font_input(config: Config) {
    FONT_INPUT.with(|b| {
        plugin_config::set_input(
            b,
            config
                .font_file
                .as_ref()
                .map_or("default", FontFile::filename),
        )
    });
}

// 游戏内存读取、本地玩家识别和坐标投影。
unsafe fn read_u32(address: u32) -> u32 {
    unsafe { ptr::read_volatile(address as *const u32) }
}

unsafe fn read_u8(address: u32) -> u8 {
    unsafe { ptr::read_volatile(address as *const u8) }
}

unsafe fn diagnose_globals(base: u32) {
    // Read only the slots themselves. Do not dereference the returned pointers
    // and do not invoke the indexed getter in diagnostic mode.
    let player_manager = unsafe { read_u32(base + 0x018A6FA8) };
    let player_neighbor_4 = unsafe { read_u32(base + 0x018A6FAC) };
    let player_neighbor_C = unsafe { read_u32(base + 0x018A6FB4) };
    let getter_context = unsafe { read_u32(base + 0x0185A990) };
    let camera_manager = unsafe { read_u32(base + 0x0186F750) };
    let ui_manager = unsafe { read_u32(base + 0x0214868C) };
    let graphics_manager = unsafe { read_u32(base + 0x02148998) };

    info!("mhxx-jp-1.5.1 diagnostic: main/text base=0x{base:08X}");
    info!("mhxx-jp-1.5.1 diagnostic: player slot +0x018A6FA8=0x{player_manager:08X}; adjacent +4=0x{player_neighbor_4:08X}, +0xC=0x{player_neighbor_C:08X}");
    info!("mhxx-jp-1.5.1 diagnostic: getter context slot +0x0185A990=0x{getter_context:08X}");
    info!("mhxx-jp-1.5.1 diagnostic: camera slot +0x0186F750=0x{camera_manager:08X}; UI slot +0x0214868C=0x{ui_manager:08X}; graphics slot +0x02148998=0x{graphics_manager:08X}");
    info!("mhxx-jp-1.5.1 diagnostic: hooks were intentionally NOT installed");
}

unsafe fn diagnose_runtime_getter(base: u32) {
    // Log every 300 update callbacks (~5 seconds at 60 FPS); delay the first
    // call so the title/loading transition has time to settle.
    let frame = JP_DIAGNOSTIC_FRAME.fetch_add(1, Ordering::Relaxed);
    if frame == 0 || frame % 300 != 0 {
        return;
    }

    let context = unsafe { read_u32(base + 0x018A6FA8) };
    let alternative = unsafe { read_u32(base + 0x0185A990) };
    if context < 0x40000000 || context >= 0x60000000 || context & 3 != 0 {
        info!("mhxx-jp-1.5.1 runtime getter probe: skip; candidate context=0x{context:08X} fails heap-range/alignment check; alternative=0x{alternative:08X}");
        return;
    }

    // This is the function and call shape used by the original project's
    // local_player(): getter(context). No hooks are installed and no memory is
    // written. The return value is logged only; it is not dereferenced.
    let getter: unsafe extern "C" fn(u32) -> u32 =
        unsafe { mem::transmute(base + 0x002734D0) };
    let result = unsafe { getter(context) };
    let pointer_like = result >= 0x40000000 && result < 0x60000000 && result & 3 == 0;
    // Original MHGU uses player+0x1054 as a room/epoch byte. Read only this
    // known field when the returned pointer passes the basic range/alignment guard.
    // The guard is heuristic; it does not prove that the address is mapped.
    let (room_field, weight_slash, weight_blunt, flags_13cc) = if pointer_like {
        // These fields are read by the original mod when classifying damage:
        // +0x1054 is its room/epoch byte; +0x2B70/+0x2B71 are hitzone weights;
        // +0x13CC contains a flag used to select a physical damage channel.
        // The range/alignment check is heuristic, not a proof of mapped memory.
        unsafe {
            (
                read_u8(result + 0x1054),
                read_u8(result + 0x2B70),
                read_u8(result + 0x2B71),
                read_u8(result + 0x13CC),
            )
        }
    } else {
        (0xFF, 0xFF, 0xFF, 0xFF)
    };
    let weights_plausible = weight_slash <= 100 && weight_blunt <= 100;
    info!("mhxx-jp-1.5.1 player-layout probe: frame={frame} context=0x{context:08X} result=0x{result:08X} heap_like={pointer_like} room_field_1054=0x{room_field:02X} weights_2B70_2B71=0x{weight_slash:02X},0x{weight_blunt:02X} weights_le_100={weights_plausible} flags_13CC=0x{flags_13cc:02X} alternative_slot_value=0x{alternative:08X}");
}

fn thread_key() -> u32 {
    let value: u32;
    // AArch32 TPIDRURO：只用 TLS 地址区分调用线程，不解引用 OS 内部结构。
    unsafe {
        asm!("mrc p15, 0, {value}, c13, c0, 3", value = out(reg) value, options(nostack, preserves_flags))
    };
    value
}

unsafe fn local_player(base: u32) -> u32 {
    let get: unsafe extern "C" fn(u32) -> u32 = unsafe { mem::transmute(base + 0x002734D0) };
    unsafe { get(read_u32(base + 0x018A6FA8)) }
}

unsafe fn epoch(player: u32) -> Epoch {
    Epoch {
        player,
        room: unsafe { read_u8(player + 0x1054) },
    }
}

unsafe fn display_area(base: u32) -> DisplayArea {
    // 图形管理器 +B0/+B4 同时用于游戏绘图和 nvnWindowSetCrop 的宽高。
    // +00BC0874 将它们传给 +0085EC7C；裁剪原点固定为 (0, 0)。
    // 每次投影读取当前值，掌机/主机切换后同步更新插件快照。
    let graphics = unsafe { read_u32(base + 0x02148998) };
    DisplayArea {
        width: unsafe { read_u32(graphics + 0xB0) } as f32,
        height: unsafe { read_u32(graphics + 0xB4) } as f32,
    }
}

unsafe fn damage_is_yellow(base: u32, contact: Contact) -> bool {
    // 在伤害入账时读取已完成设置的攻击字段，不能提前按武器或外观猜瓶爆。
    let flags = unsafe { read_u32(contact.attack + 0x30) };
    if flags & 0x18021 == 0 {
        return true;
    }
    let get_hitzones: unsafe extern "C" fn(u32, u32) -> u32 =
        unsafe { mem::transmute(base + 0x000B989C) };
    let table = unsafe { get_hitzones(contact.monster, contact.part_index as u32) };
    let hitzones = unsafe { [read_u8(table), read_u8(table + 1), read_u8(table + 2)] };
    let weights = if flags & 0x20 != 0 {
        let player = contact.epoch.player;
        let mut weights = unsafe { [read_u8(player + 0x2B70), read_u8(player + 0x2B71)] };
        // +13CC bit2 的原结算分支只传入本次指定的斩或打权重。
        if unsafe { read_u8(player + 0x13CC) } & 4 != 0 {
            if unsafe { read_u32(contact.attack + 0x40) } & 2 != 0 {
                weights[1] = 0;
            } else {
                weights[0] = 0;
            }
        }
        weights
    } else if flags & 0x10000 != 0 {
        if unsafe { read_u32(contact.attack + 0x40) } & 2 != 0 {
            [100, 0]
        } else {
            [0, 100]
        }
    } else {
        // +30 的 0x8000 分支给斩、打都传入 100；弹分支不使用这两个值。
        [100, 100]
    };
    yellow_for_hitzone(flags, hitzones, weights)
}

impl Projection {
    pub unsafe fn current(base: u32) -> Self {
        let manager = unsafe { read_u32(base + 0x0186F750) };
        let ui = unsafe { read_u32(base + 0x0214868C) };
        let view = unsafe { read_u8(ui + 0xEC) } as u32;
        let record = manager + view * 0x1B0;
        Self {
            function: unsafe { mem::transmute(base + 0x00018238) },
            manager,
            view,
            width: (unsafe { read_u32(record + 0x70) } as i32
                - unsafe { read_u32(record + 0x68) } as i32) as f32,
            height: (unsafe { read_u32(record + 0x74) } as i32
                - unsafe { read_u32(record + 0x6C) } as i32) as f32,
        }
    }

    pub unsafe fn project(&self, world: [f32; 3]) -> Option<[f32; 2]> {
        let world = Vector([world[0], world[1], world[2], 0.0]);
        let mut projected = Vector([0.0; 4]);
        unsafe {
            (self.function)(
                projected.0.as_mut_ptr(),
                self.manager,
                world.0.as_ptr(),
                self.view,
            )
        };
        normalized_screen(projected.0, self.width, self.height)
    }
}

fn compatible(base: u32) -> bool {
    for (offset, expected) in [
        (0x0016DF60, 0xE92D41F0),
        (0x0016DF64, 0xE24DD010),
        (0x0009C508, 0xE92D4BF0),
        (0x0009C50C, 0xED2D8B02),
        (0x00018238, 0xE92D4FF0),
        (0x0001823C, 0xE28DB01C),
        (0x002734D0, 0xE92D4010),
        (0x002734D4, 0xE1A04000),
        (0x000B989C, 0xE3510008),
        (0x000B98A0, 0x3A000002),
        (0x0085EC7C, 0xE92D4800),
        (0x0085EC80, 0xE24DD008),
    ] {
        let actual = unsafe { read_u32(base + offset) };
        if actual != expected {
            error!(
                "mhxx-jp-1.5.1-attack-damage-display: main+0x{offset:08X}=0x{actual:08X}, expected 0x{expected:08X}; hooks skipped"
            );
            return false;
        }
    }
    true
}

// 当前加载器的 76 字节 Hook 返回类型在 data.rs 定义。
unsafe extern "C" {
    #[link_name = "forge_hook_create"]
    fn create_hook_raw(
        output: *mut RuntimeHook,
        target: u32,
        detour: u32,
        original: *mut u32,
    ) -> *mut RuntimeHook;
}

// 每次原命中调用绑定栈上下文；嵌套命中结束后恢复上一层。
// 显式清零最大 SRET 缓冲，兼容当前 24 字节与旧版 76 字节返回区。
unsafe fn create_hook(target: u32, detour: u32, original: *mut u32) -> RuntimeHook {
    let mut hook: RuntimeHook = unsafe { core::mem::zeroed() };
    unsafe { create_hook_raw(&raw mut hook, target, detour, original) };
    hook
}

unsafe fn install_damage_probe(base: u32) {
    // Install a trampoline around the JP 1.5.1 candidate only. The damage
    // display pipeline stays disabled because DIAGNOSTIC_ONLY returns before
    // config loading and ENABLED remains false.
    unsafe {
        ptr::write(
            &raw mut DAMAGE_HOOK,
            MaybeUninit::new(create_hook(
                base + 0x0009C508,
                damage as *const () as u32,
                &raw mut DAMAGE_ORIGINAL,
            )),
        );
    }
    info!("mhxx-jp-1.5.1 passive damage probe installed at main+0x0009C508; damage-number pipeline remains disabled");
}

unsafe fn install_collision_probe(base: u32) {
    // Reuse the project's Hook wrapper/return storage, but install ONLY the
    // collision callback. DAMAGE_HOOK is deliberately left untouched.
    unsafe {
        ptr::write(
            &raw mut COLLISION_HOOK,
            MaybeUninit::new(create_hook(
                base + 0x0016DF60,
                collision as *const () as u32,
                &raw mut COLLISION_ORIGINAL,
            )),
        );
    }
    info!("mhxx-jp-1.5.1 collision-only probe installed at main+0x0016DF60; damage hook remains disabled");
}

unsafe fn install_hooks(base: u32) {
    // 由加载器初始化阶段安装；整个进程生命周期内保留两个 JIT 跳板。
    unsafe {
        ptr::write(
            &raw mut DAMAGE_HOOK,
            MaybeUninit::new(create_hook(
                base + 0x0009C508,
                damage as *const () as u32,
                &raw mut DAMAGE_ORIGINAL,
            )),
        );
        ptr::write(
            &raw mut COLLISION_HOOK,
            MaybeUninit::new(create_hook(
                base + 0x0016DF60,
                collision as *const () as u32,
                &raw mut COLLISION_ORIGINAL,
            )),
        );
    }
}

unsafe extern "C" fn collision(
    check: u32,
    attack: u32,
    monster: u32,
    point: u32,
    body: u32,
    attack_point: u32,
    part: u32,
    shape: u32,
) -> u32 {
    let base = GAME_BASE.load(Ordering::Relaxed);
    // Sample sparsely: log the first 8 callbacks and then every 4096th call.
    // All callback parameters are logged as raw values; none are dereferenced.
    let probe_call = JP_COLLISION_PROBE_CALLS.fetch_add(1, Ordering::Relaxed);
    if probe_call < 8 || probe_call % 4096 == 0 {
        let player = unsafe { local_player(base) };
        info!("mhxx-jp-1.5.1 collision-only probe: call={probe_call} check=0x{check:08X} attack=0x{attack:08X} monster=0x{monster:08X} point=0x{point:08X} body=0x{body:08X} attack_point=0x{attack_point:08X} part=0x{part:08X} shape=0x{shape:08X} local_player=0x{player:08X}");
    }
    let mut contact = Contact {
        enabled: false,
        attack,
        monster,
        slot: 0,
        part_index: 0,
        epoch: Epoch { player: 0, room: 0 },
        world: [0.0; 3],
    };
    if ENABLED.load(Ordering::Relaxed) && unsafe { read_u8(attack + 0x6D) } == 1 {
        let forwarded = unsafe { read_u8(attack + 0x10) } != 0;
        if !forwarded || unsafe { read_u8(attack + 0x6E) } == 0 {
            let owner = unsafe { read_u32(attack + if forwarded { 0x70 } else { 0x0C }) };
            let player = unsafe { local_player(base) };
            if owner == player {
                contact.enabled = true;
                contact.slot = unsafe { read_u8(part + 8) } as u32;
                contact.part_index = unsafe { read_u8(part + 6) };
                contact.epoch = unsafe { epoch(player) };
                contact.world = unsafe { ptr::read_unaligned(point as *const [f32; 3]) };
                TOTAL_HITS.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    let thread = thread_key();
    let binding = BINDINGS.with(|bindings| {
        let index = bindings
            .0
            .iter()
            .position(|entry| entry.thread == thread)
            .or_else(|| bindings.0.iter().position(|entry| entry.thread == 0))?;
        let previous = bindings.0[index];
        bindings.0[index] = Binding {
            thread,
            contact: &contact,
        };
        Some((index, previous))
    });
    let original: unsafe extern "C" fn(u32, u32, u32, u32, u32, u32, u32, u32) -> u32 =
        unsafe { mem::transmute(COLLISION_ORIGINAL) };
    let result = unsafe {
        original(
            check,
            attack,
            monster,
            point,
            body,
            attack_point,
            part,
            shape,
        )
    };
    if let Some((index, previous)) = binding {
        BINDINGS.with(|bindings| bindings.0[index] = previous);
    }
    result
}

unsafe extern "C" fn damage(monster: u32, slot: u32, raw: i32, mode: i32) -> i32 {
    let entry_thread = thread_key();
        let critical_state_at_entry = BINDINGS.with(|bindings| {
            bindings
                .0
                .iter()
                .find(|entry| entry.thread == entry_thread)
                .map(|entry| unsafe { *entry.contact })
        })
        .filter(|contact| {
            contact.enabled && contact.monster == monster && contact.slot == slot
        })
        .map_or(0, |contact| unsafe { read_u8(contact.attack + 0x54) });
        let original: unsafe extern "C" fn(u32, u32, i32, i32) -> i32 =
        unsafe { mem::transmute(DAMAGE_ORIGINAL) };
    let result = unsafe { original(monster, slot, raw, mode) };
    // Passive probe: log only raw inputs/output and whether this call happened
    // synchronously inside the collision-hook scope. Do not inspect either object.
    let probe_call = JP_DAMAGE_PROBE_CALLS.fetch_add(1, Ordering::Relaxed);
    if probe_call < 24 || (raw > 0 && result > 0 && probe_call % 512 == 0) {
        let thread = thread_key();
        let collision_binding = BINDINGS.with(|bindings| {
            bindings.0.iter().any(|entry| entry.thread == thread)
        });
        info!("mhxx-jp-1.5.1 damage-only probe: call={probe_call} monster=0x{monster:08X} slot={slot} raw={raw} mode={mode} result={result} collision_binding={collision_binding}");
    }
    if raw > 0 && result > 0 && mode != 2 && slot < 8 && ENABLED.load(Ordering::Relaxed) {
        let thread = thread_key();
        let contact = BINDINGS.with(|bindings| {
            bindings
                .0
                .iter()
                .find(|entry| entry.thread == thread)
                .map(|entry| unsafe { *entry.contact })
        });
        if let Some(contact) = contact {
            if contact.enabled && contact.monster == monster && contact.slot == slot {
                let yellow =
                    unsafe { damage_is_yellow(GAME_BASE.load(Ordering::Relaxed), contact) };
                STATE.with(|state| {
                    if ENABLED.load(Ordering::Relaxed)
                        && state.push_with_critical(contact.epoch, contact.world, result, yellow, critical_state_at_entry)
                    {
                        TOTAL_EVENTS.fetch_add(1, Ordering::Relaxed);
                    }
                });
            }
        }
    }
    result
}
