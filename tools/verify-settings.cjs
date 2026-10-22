// 运行真实显伤菜单、配置读写及 Hook 返回区；替身只提供输入与文件系统。
const fs=require('fs'),path=require('path'),cp=require('child_process');
const root=path.resolve(__dirname,'..'),out=path.join(root,'target/settings-check');
fs.mkdirSync(out,{recursive:true});
const rustPath=p=>JSON.stringify(p.replaceAll('\\','/'));
const modulePath=(file,name)=>`#[path=${rustPath(path.join(root,file))}] mod ${name};`;
let mock=fs.readFileSync(path.join(__dirname,'settings-host.rs'),'utf8');
mock=mock.slice(0,mock.indexOf('fn click(')).replace('LOCATION.saved','LOCATION.path')
 .replace('pub struct ImGuiTreeNodeFlags;',`#[derive(Default)] pub struct ImVec2; impl ImVec2 {pub fn new(_:f32,_:f32)->Self{Self}}
 pub struct ImGuiTreeNodeFlags;`)
 .replace('impl ImGuiInputTextFlags { pub const None: Self = Self; }','impl ImGuiInputTextFlags { pub const None: Self = Self; pub const AllowTabInput:Self=Self; }')
 .replace('button: String, stack:', 'input: Option<String>, colors:BTreeMap<String,[f32;4]>, button: String, stack:')
 .replace('pub fn text(text: &str)',`pub fn color_edit4(label:&CStr,value:&mut [f32;4],flags:sys::imgui::ImGuiColorEditFlags)->bool {
 assert_ne!(flags.0 & sys::imgui::ImGuiColorEditFlags::DisplayHex.0,0,"color must use HEX input");
 CONTROLS.with(|c|if let Some(v)=c.borrow_mut().colors.remove(label.to_str().unwrap()){*value=v;true}else{false})}
 pub fn input_text(_: &CStr,buffer:&mut[u8],_:sys::imgui::ImGuiInputTextFlags)->bool {
 CONTROLS.with(|c|if let Some(v)=c.borrow_mut().input.take(){buffer.fill(0);buffer[..v.len()].copy_from_slice(v.as_bytes());true}else{false})}
 pub fn input_text_multiline(label:&CStr,buffer:&mut[u8],_:sys::imgui::ImVec2,flags:sys::imgui::ImGuiInputTextFlags)->bool{input_text(label,buffer,flags)}
 pub fn text(text: &str)`);

const lib=fs.readFileSync(path.join(root,'src/lib.rs'),'utf8');
const reader=lib.slice(lib.indexOf('fn load_config()'),lib.indexOf('fn file_exists('));
const ui=lib.slice(lib.indexOf('#[forge::imgui_render]'),lib.indexOf('// 游戏内存读取')).replaceAll('#[forge::imgui_render]','');
const start=lib.indexOf('unsafe fn create_hook('),end=lib.indexOf('\n}\n',start)+3;
const declaration=lib.match(/unsafe extern "C" \{\s*#\[link_name = "forge_hook_create"\][\s\S]*?\n\}/);
if(start<0||end<=start||!declaration)throw Error('SRET helper missing');
const source=`#![allow(dead_code,unused_imports,non_upper_case_globals,private_interfaces)]
extern crate self as forge;
use core::sync::atomic::{AtomicBool,AtomicU32,Ordering};
use forge::log::prelude::warn;
${modulePath('src/data.rs','data')} use data::*;
${modulePath('shared/plugin_config.rs','plugin_config')}
use plugin_config::DAMAGE as LOCATION;
const DIAGNOSTIC_ONLY: bool = false; // host harness tests normal settings UI
use sys::imgui::ImGuiInputTextFlags;
${mock}
static MENU:plugin_config::Shared<plugin_config::Edit<Config>>=plugin_config::Shared::new(plugin_config::Edit::new(Config::DEFAULT));
static FONT_INPUT:plugin_config::Shared<[u8;168]>=plugin_config::Shared::new([0;168]);
${reader}
${ui}
${declaration[0]}
${lib.slice(start,end)}
static RETURN_BYTES:AtomicU32=AtomicU32::new(24);
#[unsafe(export_name="forge_hook_create")]
extern "C" fn mock_hook(out:*mut u8,target:u32,detour:u32,_original:*mut c_void)->*mut u8{
 assert_eq!((target,detour),(0x10,0x20));unsafe{std::ptr::write_bytes(out,0xAB,RETURN_BYTES.load(Ordering::Relaxed) as usize)};out
}
fn read_back()->String{STORAGE.with(|s|std::str::from_utf8(&s.borrow().files[&saved_path()]).unwrap().to_owned())}
fn click(label:&str){CONTROLS.with(|c|c.borrow_mut().button=label.into());draw_settings();}
fn main(){
 assert_eq!(load_config(),Config::DEFAULT);assert_eq!(Config::parse(&read_back()).unwrap(),Config::DEFAULT);
 STORAGE.with(|s|assert_eq!(s.borrow().writes,1));load_config();STORAGE.with(|s|{let mut s=s.borrow_mut();assert_eq!(s.writes,1);s.writes=0;});

     INSTALLED.store(true,Ordering::Release);set_font_input(Config::DEFAULT);
     CONTROLS.with(|c|{let mut c=c.borrow_mut();c.floats.insert("Font size".into(),48.0);c.floats.insert("Full opacity (seconds)".into(),2.0);c.floats.insert("Total lifetime (seconds)".into(),3.0);c.colors.insert("Normal damage color".into(),[1.0,0.0,0.0,0.5]);c.input=Some("中文字体.ttf".into());});
     draw_settings();let live=STATE.with(|s|s.config);assert_eq!(live.font_size,48.0);assert_eq!(live.normal_color,[255,0,0,128]);STORAGE.with(|s|assert_eq!(s.borrow().writes,0));
     click("Save INI");assert_eq!(Config::parse(&read_back()).unwrap(),live);
     let writes=STORAGE.with(|s|s.borrow().writes);
     CONTROLS.with(|c|c.borrow_mut().floats.insert("Full opacity (seconds)".into(),4.0));draw_settings();assert_eq!(STATE.with(|s|s.config),live);click("Save INI");assert_eq!(STORAGE.with(|s|s.borrow().writes),writes);
     click("Revert changes");CONTROLS.with(|c|c.borrow_mut().input=Some("bad#font.ttf".into()));draw_settings();click("Save INI");assert_eq!(STORAGE.with(|s|s.borrow().writes),writes);
     click("Revert changes");click("Defaults");assert_eq!(STATE.with(|s|s.config),Config::DEFAULT);
     STORAGE.with(|s|s.borrow_mut().write_error=true);click("Save INI");assert_eq!(MENU.with(|m|m.saved),live);
     STORAGE.with(|s|s.borrow_mut().write_error=false);click("Revert changes");click("Save INI");assert_eq!(Config::parse(&read_back()).unwrap(),live);
     
 for count in [24,76]{RETURN_BYTES.store(count,Ordering::Relaxed);let mut original=0;
 let hook=unsafe{create_hook(0x10,0x20,&raw mut original)};
 let bytes=unsafe{std::slice::from_raw_parts((&raw const hook).cast::<u8>(),core::mem::size_of_val(&hook))};
 assert_eq!(bytes.len(),76);assert!(bytes[..count as usize].iter().all(|b|*b==0xAB));assert!(bytes[count as usize..].iter().all(|b|*b==0));}
 STORAGE.with(|s|{let s=s.borrow();assert_eq!(s.handles,0);assert_eq!(s.active_mounts,0);});
 println!("PASS: real damage settings; automatic save-root INI, HEX RGBA, preview/save/revert/defaults/errors, 24/76-byte Hook returns");
}`;
const file=path.join(out,'settings-host.rs'),exe=path.join(out,'settings-host.exe');
fs.writeFileSync(file,source);
cp.execFileSync(process.env.RUSTC||'rustc',['--edition=2024',file,'-o',exe],{stdio:'inherit'});
cp.execFileSync(exe,{stdio:'inherit'});
