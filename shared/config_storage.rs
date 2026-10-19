//! 本插件只读写当前 MHGU 用户的存档根目录；缺少文件时生成随插件内置的默认 INI。
#![allow(dead_code)]

use core::ffi::{CStr, c_char, c_void};
use forge::log::prelude::{info, warn};

pub const MAX_BYTES: usize = 8192;

pub struct Location {
    pub path: &'static CStr,
    pub display: &'static str,
    pub defaults: &'static str,
}
macro_rules! location {
    ($name:ident, $file:literal, $defaults:literal) => {
        pub const $name: Location = Location {
            path: unsafe {
                CStr::from_bytes_with_nul_unchecked(concat!("savedata:/", $file, "\0").as_bytes())
            },
            display: concat!("MHGU save: /", $file),
            defaults: include_str!($defaults),
        };
    };
}
location!(
    DAMAGE,
    "attack_damage_display.ini",
    "../conf/attack_damage_display.ini"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    Unavailable,
    Io,
    TooLarge,
    Utf8,
}

// GU ARM32 SDK 的 FileHandle 是四字节，WriteOption 按引用传递。
#[repr(C)]
#[derive(Clone, Copy)]
struct FileHandle(u32);
#[repr(C)]
struct WriteOption(i32);
const _: () = assert!(core::mem::size_of::<FileHandle>() == 4);
unsafe extern "C" {
    fn forge_fs_mountSaveData() -> bool;
    fn forge_fs_unmountSaveData();
    #[link_name = "_ZN2nn2fs14CommitSaveDataEPKc"]
    fn commit_save(name: *const c_char) -> u32;
    #[link_name = "_ZN2nn2fs8OpenFileEPNS0_10FileHandleEPKci"]
    fn open_file(handle: *mut FileHandle, path: *const c_char, mode: i32) -> u32;
    #[link_name = "_ZN2nn2fs11GetFileSizeEPxNS0_10FileHandleE"]
    fn get_file_size(size: *mut i64, handle: FileHandle) -> u32;
    #[link_name = "_ZN2nn2fs8ReadFileENS0_10FileHandleExPvj"]
    fn read_file(handle: FileHandle, offset: i64, bytes: *mut c_void, size: usize) -> u32;
    #[link_name = "_ZN2nn2fs10CreateFileEPKcx"]
    fn create_file(path: *const c_char, size: i64) -> u32;
    #[link_name = "_ZN2nn2fs11SetFileSizeENS0_10FileHandleEx"]
    fn set_file_size(handle: FileHandle, size: i64) -> u32;
    #[link_name = "_ZN2nn2fs9WriteFileENS0_10FileHandleExPKvjRKNS0_11WriteOptionE"]
    fn write_file(
        handle: FileHandle,
        offset: i64,
        bytes: *const c_void,
        size: usize,
        option: *const WriteOption,
    ) -> u32;
    #[link_name = "_ZN2nn2fs9CloseFileENS0_10FileHandleE"]
    fn close_file(handle: FileHandle);
}

struct SaveMount;
impl SaveMount {
    fn open() -> Result<Self, ReadError> {
        if unsafe { forge_fs_mountSaveData() } {
            Ok(Self)
        } else {
            warn!("Could not mount MHGU savedata");
            Err(ReadError::Unavailable)
        }
    }
}
impl Drop for SaveMount {
    fn drop(&mut self) {
        // 使用游戏自己的挂载计数；文件句柄在此之前全部关闭。
        unsafe { forge_fs_unmountSaveData() };
    }
}

fn missing(code: u32) -> bool {
    // nn::fs::ResultPathNotFound，不能把权限／挂载错误当成缺少文件。
    code & 0x3F_FFFF == 0x202
}

impl Location {
    pub fn read<'a>(&self, bytes: &'a mut [u8]) -> Result<&'a str, ReadError> {
        let _mount = SaveMount::open()?;
        let mut file = FileHandle(0);
        let code = unsafe { open_file(&mut file, self.path.as_ptr(), 1) };
        if missing(code) {
            if self.defaults.len() > bytes.len() {
                return Err(ReadError::TooLarge);
            }
            self.write_mounted(self.defaults)
                .map_err(|_| ReadError::Io)?;
            bytes[..self.defaults.len()].copy_from_slice(self.defaults.as_bytes());
            info!("{}: generated default INI", self.display);
            return Ok(core::str::from_utf8(&bytes[..self.defaults.len()]).unwrap());
        }
        if code != 0 {
            warn!("{}: open failed 0x{code:08X}", self.display);
            return Err(ReadError::Io);
        }
        let mut size = 0i64;
        let mut code = unsafe { get_file_size(&mut size, file) };
        if code == 0 && (0..=bytes.len() as i64).contains(&size) {
            code = unsafe { read_file(file, 0, bytes.as_mut_ptr().cast(), size as usize) };
        }
        unsafe { close_file(file) };
        if code != 0 {
            warn!("{}: read failed 0x{code:08X}", self.display);
            return Err(ReadError::Io);
        }
        if !(0..=bytes.len() as i64).contains(&size) {
            return Err(ReadError::TooLarge);
        }
        let text = core::str::from_utf8(&bytes[..size as usize]).map_err(|_| ReadError::Utf8)?;
        info!("{}: loaded", self.display);
        Ok(text)
    }

    pub fn load<C: Copy, E: core::fmt::Debug>(
        &self,
        default: C,
        parse: impl FnOnce(&str) -> Result<C, E>,
    ) -> C {
        let mut bytes = [0; MAX_BYTES];
        match self.read(&mut bytes) {
            Ok(text) => match parse(text) {
                Ok(config) => return config,
                Err(problem) => warn!("{}: {problem:?}; using defaults", self.display),
            },
            Err(problem) => warn!("{}: {problem:?}; using defaults", self.display),
        }
        default
    }

    pub fn save(&self, text: &str) -> Result<(), &'static str> {
        let _mount =
            SaveMount::open().map_err(|_| "Could not mount MHGU save data. See Forge log.")?;
        self.write_mounted(text)
    }

    pub fn save_text(&self, text: &str) -> Result<(), &'static str> {
        self.save(text)
    }

    fn write_mounted(&self, text: &str) -> Result<(), &'static str> {
        let mut file = FileHandle(0);
        let code = unsafe { open_file(&mut file, self.path.as_ptr(), 6) };
        if missing(code) {
            let code = unsafe { create_file(self.path.as_ptr(), 0) };
            if code != 0 {
                return self.failed(code, "Could not create save INI. See Forge log.");
            }
            let code = unsafe { open_file(&mut file, self.path.as_ptr(), 6) };
            if code != 0 {
                return self.failed(code, "Could not open save INI for writing.");
            }
        } else if code != 0 {
            return self.failed(code, "Could not open save INI for writing.");
        }
        let mut code = unsafe { set_file_size(file, text.len() as i64) };
        if code == 0 {
            code =
                unsafe { write_file(file, 0, text.as_ptr().cast(), text.len(), &WriteOption(1)) };
        }
        unsafe { close_file(file) };
        if code != 0 {
            return self.failed(code, "Could not write save INI. See Forge log.");
        }
        // Forge 的释放接口也会提交；先检查一次结果，保存按钮才能准确报告提交失败。
        let code = unsafe { commit_save(c"savedata".as_ptr()) };
        if code != 0 {
            return self.failed(code, "Could not commit MHGU save INI. See Forge log.");
        }
        info!("{}: saved", self.display);
        Ok(())
    }

    fn failed(&self, code: u32, message: &'static str) -> Result<(), &'static str> {
        warn!("{}: save failed 0x{code:08X}", self.display);
        Err(message)
    }
}
