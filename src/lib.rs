//! MHGU 1.4.0 本地伤害数字：采集命中 → 伤害入账 → 游戏投影 → Forge 绘制。
#![no_std]

mod data;

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
    mt::{
        fs::{MtFile, OpenMode},
        object::Object,
    },
    sys::imgui::ImVec2,
};

// Forge 入口、更新和绘图。
// 字体仅在 ImGui 初始化时写入，随后由同一渲染线程读取。
static mut CUSTOM_FONT: Option<imgui::ImFont> = None;

#[forge::entry]
fn main() {
    let base = forge::mem::text_addr();
    GAME_BASE.store(base, Ordering::Relaxed);
    if compatible(base) {
        let config = load_config();
        STATE.with(|state| state.config = config);
        unsafe { install_hooks(base) };
        INSTALLED.store(true, Ordering::Release);
        ENABLED.store(config.enabled, Ordering::Relaxed);
        info!("mhgu-attack-damage-display v0.3: local attack damage, main=0x{base:08X}");
    }
}

fn load_config() -> Config {
    let path = CONFIG_PATH.to_str().unwrap();
    if !file_exists(CONFIG_PATH) {
        warn!("mhgu-attack-damage-display: {path} missing; using defaults");
        return Config::DEFAULT;
    }
    let result = (|| {
        let failure = |message| ConfigError { line: 0, message };
        let file = MtFile::open(CONFIG_PATH, OpenMode::Read)
            .map_err(|_| failure("could not open configuration"))?;
        let mut bytes = [0; CONFIG_MAX_BYTES];
        let length = file.size();
        let read = if length <= bytes.len() {
            file.read(&mut bytes[..length])
        } else {
            0
        };
        file.close();
        file.destroy();
        if length > bytes.len() {
            return Err(failure("configuration exceeds 4096 bytes"));
        }
        if read != length {
            return Err(failure("could not read the complete configuration"));
        }
        let text = core::str::from_utf8(&bytes[..length])
            .map_err(|_| failure("save configuration as UTF-8"))?;
        Config::parse(text)
    })();
    match result {
        Ok(config) => {
            info!("mhgu-attack-damage-display: loaded {path}");
            config
        }
        Err(error) => {
            warn!(
                "mhgu-attack-damage-display: {path}, line {}: {}; using defaults",
                error.line, error.message
            );
            Config::DEFAULT
        }
    }
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
                    "mhgu-attack-damage-display: loaded font {}",
                    path.to_str().unwrap()
                );
            }
            None => warn!(
                "mhgu-attack-damage-display: could not load font {}; using Forge font",
                path.to_str().unwrap()
            ),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn forge_onUpdate(_dt: f32) {
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
    let (display, seconds) = {
        let io = imgui::get_io();
        (io.DisplaySize, io.DeltaTime)
    };
    let seconds = if seconds.is_finite() && seconds > 0.0 {
        seconds
    } else {
        1.0 / 60.0
    };
    let snapshot = STATE.with(|state| state.advance(seconds));
    let config = snapshot.config;
    let draw = imgui::DrawList::foreground();
    let font = unsafe { CUSTOM_FONT }.unwrap_or_else(imgui::ImFont::current);
    let scale = display.y / 720.0;
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
        let x = event.screen[0] * display.x - width * 0.5
            + ((event.serial % 5) as f32 - 2.0) * 6.0 * scale;
        let y = event.screen[1] * display.y
            - size
            - ((event.age / config.full_opacity).min(1.0) * 30.0 + (event.serial % 3) as f32 * 5.0)
                * scale;
        let alpha = event.alpha(&config);
        let outline = imgui::color(
            config.outline_color[0],
            config.outline_color[1],
            config.outline_color[2],
            alpha,
        );
        let edge = config.outline_width * scale;
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
        let rgb = if event.yellow {
            config.highlight_color
        } else {
            config.normal_color
        };
        let color = imgui::color(rgb[0], rgb[1], rgb[2], alpha);
        draw.add_text_with_font(&font, size, ImVec2::new(x, y), color, text);
        visible += 1;
    }
    imgui::pop_font();
    VISIBLE.store(visible, Ordering::Relaxed);
}

#[forge::imgui_render]
fn draw_settings() {
    imgui::separator_text(c"MHGU Attack Damage Display v0.3");
    if !INSTALLED.load(Ordering::Acquire) {
        imgui::text("Hook addresses did not match. See Forge log.");
        return;
    }
    let mut enabled = ENABLED.load(Ordering::Relaxed);
    if imgui::checkbox(c"Show attack damage", &mut enabled) {
        ENABLED.store(enabled, Ordering::Relaxed);
        STATE.with(|state| state.clear());
    }
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

// 游戏内存读取、本地玩家识别和坐标投影。
unsafe fn read_u32(address: u32) -> u32 {
    unsafe { ptr::read_volatile(address as *const u32) }
}

unsafe fn read_u8(address: u32) -> u8 {
    unsafe { ptr::read_volatile(address as *const u8) }
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
    let get: unsafe extern "C" fn(u32) -> u32 = unsafe { mem::transmute(base + 0x00276278) };
    unsafe { get(read_u32(base + 0x01885B58)) }
}

unsafe fn epoch(player: u32) -> Epoch {
    Epoch {
        player,
        room: unsafe { read_u8(player + 0x1054) },
    }
}

unsafe fn damage_is_yellow(base: u32, contact: Contact) -> bool {
    // 在伤害入账时读取已完成设置的攻击字段，不能提前按武器或外观猜瓶爆。
    let flags = unsafe { read_u32(contact.attack + 0x30) };
    if flags & 0x18021 == 0 {
        return true;
    }
    let get_hitzones: unsafe extern "C" fn(u32, u32) -> u32 =
        unsafe { mem::transmute(base + 0x000BAB60) };
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
        let manager = unsafe { read_u32(base + 0x0184D750) };
        let ui = unsafe { read_u32(base + 0x0212668C) };
        let view = unsafe { read_u8(ui + 0xEC) } as u32;
        let record = manager + view * 0x1B0;
        Self {
            function: unsafe { mem::transmute(base + 0x00018A48) },
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
        (0x00170968, 0xE92D41F0),
        (0x0017096C, 0xE24DD010),
        (0x0009D6F8, 0xE92D4BF0),
        (0x0009D6FC, 0xED2D8B02),
        (0x00018A48, 0xE92D4FF0),
        (0x00018A4C, 0xE28DB01C),
        (0x00276278, 0xE92D4010),
        (0x0027627C, 0xE1A04000),
        (0x000BAB60, 0xE3510008),
        (0x000BAB64, 0x3A000002),
    ] {
        let actual = unsafe { read_u32(base + offset) };
        if actual != expected {
            error!(
                "mhgu-attack-damage-display: main+0x{offset:08X}=0x{actual:08X}, expected 0x{expected:08X}; hooks skipped"
            );
            return false;
        }
    }
    true
}

// 当前加载器的 76 字节 Hook 返回类型在 data.rs 定义。
unsafe extern "C" {
    #[link_name = "forge_hook_create"]
    fn create_hook(target: u32, detour: u32, original: *mut u32) -> RuntimeHook;
}

// 每次原命中调用绑定栈上下文；嵌套命中结束后恢复上一层。
unsafe fn install_hooks(base: u32) {
    // 由加载器初始化阶段安装；整个进程生命周期内保留两个 JIT 跳板。
    unsafe {
        ptr::write(
            &raw mut DAMAGE_HOOK,
            MaybeUninit::new(create_hook(
                base + 0x0009D6F8,
                damage as *const () as u32,
                &raw mut DAMAGE_ORIGINAL,
            )),
        );
        ptr::write(
            &raw mut COLLISION_HOOK,
            MaybeUninit::new(create_hook(
                base + 0x00170968,
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
    let original: unsafe extern "C" fn(u32, u32, i32, i32) -> i32 =
        unsafe { mem::transmute(DAMAGE_ORIGINAL) };
    let result = unsafe { original(monster, slot, raw, mode) };
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
                        && state.push(contact.epoch, contact.world, result, yellow)
                    {
                        TOTAL_EVENTS.fetch_add(1, Ordering::Relaxed);
                    }
                });
            }
        }
    }
    result
}
