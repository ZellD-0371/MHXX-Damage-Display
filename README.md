# MHGU Attack Damage Display

为《怪物猎人 XX / Generations Ultimate》增加攻击伤害数字，在命中位置附近显示本地猎人的伤害。适用于 **MHGU 1.4.0**，需要先安装与本项目配套的 Forge 前置。

## 功能与改动

- 每次有效命中独立显示数字，支持多段攻击与猎人归属的瓶爆。
- 物理肉质 **≥45** 默认黄字，**<45** 默认白字；无视物理肉质的伤害使用黄字，例如榴弹瓶伤。
- 默认字号 34，显示 1.40 秒，前 1.10 秒完全不透明，最后 0.30 秒淡出。
- 支持描边、颜色、字体及显示时长配置，可在 Forge 菜单保存。
- 修复掌机模式下数字偏右下的问题，无需为了显伤切换主机模式。

数字保留命中时的位置，镜头移动时会更新屏幕位置；不会持续跟随怪物的肢体移动。本版只显示本地猎人的命中伤害，不包含队友、随从和毒等持续扣血。

## 安装与使用

1. 安装配套 Forge 前置。
2. 将 `attack_damage_display.nro` 放入 MHGU Mod 的 `romfs/nativeNX/plugins/`。
3. 完全退出并重启游戏，进入任务攻击怪物。

插件目录只保留一份显伤 NRO。升级时覆盖同名文件，已有 INI 保留；移出 NRO 并重启即可停用。

## 修改配置

打开 **Forge → Plugin UI → Attack Damage Display**。外观设置立即预览，点击 **Save INI** 保存；**Revert changes** 恢复上次保存值，**Defaults** 恢复默认草稿，仍需保存。

首次运行在 **MHGU 当前用户的存档根目录**生成 `attack_damage_display.ini`，不建子目录。菜单与手动编辑使用同一文件。手动编辑 INI 或更换字体后，需要重启游戏。

| 设置 | 默认值与用途 |
| --- | --- |
| `enabled` | `true`，显示攻击伤害 |
| `font_size` | `34`，以 720p 为基准，自动适配主机模式 |
| `highlight_color` | `#FFDC50FF`，高肉质及无视物理肉质伤害的颜色 |
| `normal_color` | `#FFFFFFFF`，低于 45 肉质的颜色 |
| `outline_width` | `2`，描边宽度；`0` 关闭 |
| `full_opacity` | `1.10`，完全不透明的秒数 |
| `lifetime` | `1.40`，总显示秒数 |

颜色使用 `#RRGGBBAA`，最后两位为透明度。总时长需要大于完全不透明时长。默认配置模板放在 [conf/attack_damage_display.ini](conf/attack_damage_display.ini)，编译时嵌入 NRO；游戏运行时仍读写存档根目录的同名文件。

自定义字体放入 `romfs/nativeNX/forge/fonts/`，在 `font_file` 中填写有效 TTF 的文件名；`default` 使用 Forge 默认字体。字号由本插件的设置控制。

## 编译

本项目可以单独下载编译，无需 MHGU-forge 工作区。准备 Git、Rust nightly、`rust-src` 和 devkitPro / devkitARM。Windows 使用 devkitPro 的 MSYS2 Bash，Rust MSVC 工具链需要 C++ Build Tools。

从本项目根目录执行：

```bash
rustup toolchain install nightly --component rust-src
bash "build.sh"
```

最终 NRO 为 `dist/attack_damage_display.nro`。SDK 和打包工具取自固定的公开 Git 提交，工具安装在本项目 `target/`，不需要全局安装。首次构建需要联网下载依赖；缓存齐全后使用 `bash "build.sh" --offline`。

Windows 也可执行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\build.ps1`；离线时在末尾加 `-Offline`。devkitPro 默认位置为 `C:/devkitPro`，其它位置用当前进程的 `DEVKITPRO` 指定。

## 验证与打包

安装 Node.js 后运行 `bash "test.sh"`，检查默认配置、菜单保存和 NRO 格式。可传入配套 Forge NSO 和原版 MHGU ExeFS 目录，额外核对真实导出与游戏指令：

```bash
bash "test.sh" "Forge subsdk9" "MHGU ExeFS目录"
```

测试通过后，使用 Python 3 执行 `python "tools/package.py"`，生成 `dist/attack_damage_display.zip`。也可用 `node "tools/install.cjs" "你的Mod目录/romfs/nativeNX/plugins"` 安装当前产物。安装和打包均不复制 INI，已有存档配置保留。

编译和默认测试不需要游戏本体文件。以上验证不代替实机或模拟器中的狩猎测试。
