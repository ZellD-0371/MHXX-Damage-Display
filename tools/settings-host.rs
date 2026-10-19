// 运行真实菜单与配置代码；替身仅提供控件输入、互斥锁和 NN 文件系统。
use std::{cell::RefCell, collections::BTreeMap, ffi::{CStr, c_char, c_void}};

#[derive(Default)]
struct Storage {
    files: BTreeMap<String, Vec<u8>>,
    reads: usize, writes: usize, closes: usize, mounts: usize,
    active_mounts: usize, handles: usize, commits: usize, commit_error: bool,
    open_error: bool, read_error: bool, size_error: bool, write_error: bool, create_error: bool,
    negative_size: bool, mount_error: bool,
}
thread_local! { static STORAGE: RefCell<Storage> = RefCell::new(Storage::default()); }
#[macro_export] macro_rules! error { ($($arg:tt)*) => {{ let _ = format!($($arg)*); }}; }
pub mod log { pub mod prelude { pub use crate::error; pub use crate::error as info; pub use crate::error as warn; } }
pub mod os { pub mod sync {
    pub use std::sync::OnceLock;
    pub struct Mutex(std::sync::Mutex<()>);
    impl Mutex { pub fn new() -> Self { Self(std::sync::Mutex::new(())) } }
    pub struct ScopedLock<'a>(#[allow(dead_code)] std::sync::MutexGuard<'a, ()>);
    impl<'a> ScopedLock<'a> { pub fn new(m: &'a Mutex) -> Self { Self(m.0.lock().unwrap()) } }
} }
pub mod sys { pub mod imgui {
    #[derive(Clone, Copy)] pub struct ImGuiColorEditFlags(pub u32);
    impl ImGuiColorEditFlags {
        pub const None: Self=Self(0); pub const DisplayHex: Self=Self(4194304);
        pub const Uint8: Self=Self(8388608); pub const InputRGB: Self=Self(134217728); pub const NoOptions: Self=Self(8);
    }
    impl core::ops::BitOr for ImGuiColorEditFlags { type Output=Self; fn bitor(self,r:Self)->Self {Self(self.0|r.0)} }
    pub struct ImGuiTreeNodeFlags; impl ImGuiTreeNodeFlags { pub const None: Self = Self; }
    pub struct ImGuiSliderFlags; impl ImGuiSliderFlags { pub const None: Self = Self; }
    pub struct ImGuiInputTextFlags; impl ImGuiInputTextFlags { pub const None: Self = Self; }
} }
fn path_text(p: *const c_char) -> String { unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_owned() }
fn saved_path() -> String { LOCATION.path.to_str().unwrap().to_owned() }
fn file_bytes(bytes: &[u8]) {
    STORAGE.with(|s| { *s.borrow_mut() = Storage::default(); s.borrow_mut().files.insert(saved_path(),bytes.to_vec()); });
}
#[unsafe(export_name = "forge_fs_mountSaveData")]
extern "C" fn mount() -> bool {
    STORAGE.with(|s| { let mut s = s.borrow_mut(); s.mounts += 1;
        if s.mount_error {false} else {s.active_mounts+=1;true} })
}
#[unsafe(export_name = "forge_fs_unmountSaveData")]
extern "C" fn unmount() {
    STORAGE.with(|s| {let mut s=s.borrow_mut();assert_eq!(s.handles,0);assert!(s.active_mounts>0);s.active_mounts-=1;});
}
#[unsafe(export_name = "_ZN2nn2fs14CommitSaveDataEPKc")]
extern "C" fn commit(name: *const c_char) -> u32 {
    assert_eq!(path_text(name),"savedata");STORAGE.with(|s|{let mut s=s.borrow_mut();
        assert_eq!(s.handles,0);assert!(s.active_mounts>0);s.commits+=1;u32::from(s.commit_error)})
}
#[unsafe(export_name = "_ZN2nn2fs8OpenFileEPNS0_10FileHandleEPKci")]
extern "C" fn open(handle: *mut u32, path: *const c_char, mode: i32) -> u32 {
    assert_eq!(path_text(path), saved_path()); assert!([1,6].contains(&mode));
    STORAGE.with(|s| {
        let mut s = s.borrow_mut();assert!(s.active_mounts>0);
        if s.open_error {1} else if !s.files.contains_key(&saved_path()) {0x202}
        else {s.handles+=1; unsafe { *handle = if mode==1 {0x11} else {0x22} }; 0 }
    })
}
#[unsafe(export_name = "_ZN2nn2fs10CreateFileEPKcx")]
extern "C" fn create(path: *const c_char, size: i64) -> u32 {
    assert_eq!(size, 0);
    STORAGE.with(|s| {
        let mut s = s.borrow_mut();
        if s.create_error { 1 } else { s.files.insert(path_text(path), Vec::new()); 0 }
    })
}
#[unsafe(export_name = "_ZN2nn2fs11GetFileSizeEPxNS0_10FileHandleE")]
extern "C" fn get_size(size: *mut i64, handle: u32) -> u32 {
    assert_eq!(handle,0x11);
    STORAGE.with(|s| {
        let s=s.borrow(); if s.size_error {return 1;}
        unsafe { *size=if s.negative_size {-1} else {s.files[&saved_path()].len() as i64}; } 0
    })
}
#[unsafe(export_name = "_ZN2nn2fs8ReadFileENS0_10FileHandleExPvj")]
extern "C" fn read(handle:u32, offset:i64, bytes:*mut c_void, size:usize)->u32 {
    assert_eq!(handle,0x11); assert_eq!(offset,0);
    STORAGE.with(|s| {
        let mut s=s.borrow_mut();s.reads+=1;
        if s.read_error {1} else {
            unsafe {std::slice::from_raw_parts_mut(bytes.cast::<u8>(),size)}.copy_from_slice(&s.files[&saved_path()]); 0
        }
    })
}
#[unsafe(export_name = "_ZN2nn2fs11SetFileSizeENS0_10FileHandleEx")]
extern "C" fn set_size(handle: u32, size: i64) -> u32 {
    assert_eq!(handle, 0x22);
    STORAGE.with(|s| {
        let mut s = s.borrow_mut();
        if s.size_error { 1 } else { s.files.get_mut(&saved_path()).unwrap().resize(size as usize, 0); 0 }
    })
}
#[unsafe(export_name = "_ZN2nn2fs9WriteFileENS0_10FileHandleExPKvjRKNS0_11WriteOptionE")]
extern "C" fn write(handle: u32, offset: i64, bytes: *const c_void, size: usize, flags: *const i32) -> u32 {
    assert_eq!(handle, 0x22); assert_eq!(offset, 0); assert_eq!(unsafe { *flags }, 1);
    STORAGE.with(|s| {
        let mut s = s.borrow_mut(); s.writes += 1;
        if s.write_error { 1 } else {
            let data = unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), size) };
            s.files.get_mut(&saved_path()).unwrap().copy_from_slice(data); 0
        }
    })
}
#[unsafe(export_name = "_ZN2nn2fs9CloseFileENS0_10FileHandleE")]
extern "C" fn close(handle: u32) {
    assert!([0x11,0x22].contains(&handle)); STORAGE.with(|s| {let mut s=s.borrow_mut();s.closes+=1;s.handles-=1;});
}

#[derive(Default)]
struct Controls {
    bools: BTreeMap<String, bool>, floats: BTreeMap<String, f32>, ints: BTreeMap<String, i32>,
    button: String, stack: Vec<String>, text: Vec<String>,
}
thread_local! { static CONTROLS: RefCell<Controls> = RefCell::new(Controls::default()); }
pub mod imgui {
    use super::*;
    pub enum Id<'a> { Str(&'a CStr) }
    pub fn push_id(_: Id) {} pub fn pop_id() {}
    pub fn collapsing_header(_: &CStr, _: sys::imgui::ImGuiTreeNodeFlags) -> bool { true }
    pub fn tree_node(label: &CStr) -> bool {
        CONTROLS.with(|s| s.borrow_mut().stack.push(label.to_str().unwrap().to_owned())); true
    }
    pub fn tree_pop() { CONTROLS.with(|s| { s.borrow_mut().stack.pop().unwrap(); }); }
    pub fn checkbox(label: &CStr, value: &mut bool) -> bool {
        CONTROLS.with(|s| {
            let mut s = s.borrow_mut(); let key = control_key(&s, label);
            if let Some(v) = s.bools.remove(&key) { *value = v; true } else { false }
        })
    }
    fn float(label: &CStr, value: &mut f32) -> bool {
        CONTROLS.with(|s| {
            let mut s = s.borrow_mut(); let key = control_key(&s, label);
            if let Some(v) = s.floats.remove(&key) { *value = v; true } else { false }
        })
    }
    pub fn input_float(l: &CStr, v: &mut f32, _: f32, _: f32, _: sys::imgui::ImGuiInputTextFlags) -> bool { float(l, v) }
    pub fn slider_float(l: &CStr, v: &mut f32, _: f32, _: f32, _: sys::imgui::ImGuiSliderFlags) -> bool { float(l, v) }
    pub fn slider_int(l: &CStr, value: &mut i32, _: i32, _: i32, _: sys::imgui::ImGuiSliderFlags) -> bool {
        CONTROLS.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(v) = s.ints.remove(l.to_str().unwrap()) { *value = v; true } else { false }
        })
    }
    #[derive(Default)] pub struct Size;
    pub fn button(label: &CStr, _: Size) -> bool {
        CONTROLS.with(|s| {
            let mut s = s.borrow_mut();
            if s.button == label.to_str().unwrap() { s.button.clear(); true } else { false }
        })
    }
    pub fn text(text: &str) { CONTROLS.with(|s| s.borrow_mut().text.push(text.to_owned())); }
    pub fn separator() {} pub fn separator_text(_: &CStr) {} pub fn same_line(_: f32, _: f32) {}
}
fn control_key(c: &Controls, label: &CStr) -> String {
    let name = label.to_str().unwrap();
    if let Some(task) = c.stack.iter().find(|s| s.starts_with("action.") || s.starts_with("burst.")) {
        format!("{}/{name}", task.split(':').next().unwrap())
    } else { name.to_owned() }
}
