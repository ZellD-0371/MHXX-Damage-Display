# MHGU Attack Damage Display v0.3

项目目录及 Cargo 包名为 `mhgu-attack-damage-display`，英文名称 **MHGU Attack Damage Display**（MHGU 显示攻击伤害）。独立插件文件名为 `attack_damage_display.nro`，面向本地 GU 1.4.0 的 ARM32 main 和当前安装的 Forge。用 GU 原函数投影命中点，用 Forge 的 ImGui foreground draw list 绘制文字。

用户直接编辑与 NRO 同目录的 [attack_damage_display.ini](./attack_damage_display.ini)，保存并重启游戏即可生效，**无需修改 Rust 源码或重新编译**。配置默认启用，只显示本地猎人的伤害：720p 基础字号 34 像素，八方向、2 像素黑色描边，其它分辨率按画面高度同比缩放。默认显示 1.40 秒，前 1.10 秒完全不透明，上浮约 30 像素，最后 0.30 秒停住上浮并淡出。

本次攻击使用的物理肉质 **≥45 显示黄色，<45 显示白色**。榴弹瓶等不乘斩/打/弹肉质的伤害直接显示黄色；45 本身算黄色。颜色在本次入账时确定并保存，已经弹出的数字不会随怪物后续状态变化改色。颜色不表示会心。

每次命中保留独立事件，最多同时 64 条；容量满时覆盖最早的事件。同一攻击的多段命中、瓶爆不做去重。数字锚定命中时的世界点，随后随相机重新投影；本版不随怪物骨骼移动。

## 修改配置文件

在记事本等文本编辑器里打开**游戏插件目录中的** `attack_damage_display.ini`，修改数值并保存为 UTF-8（支持 UTF-8 BOM）。本机实际文件：

```text
C:/Users/Lawrence/AppData/Roaming/Ryujinx/mods/contents/0100770008dd8000/romfs/nativeNX/plugins/attack_damage_display.ini
```

配置内容如下，配套文件还带有每一项的中文注释：

```ini
[display]
enabled = true
font_size = 34.0
font_file = default
highlight_color = 255, 220, 80
normal_color = 255, 255, 255
outline_color = 10, 10, 10
outline_width = 2.0
full_opacity = 1.10
lifetime = 1.40
```

| 参数 | 如何修改 |
| --- | --- |
| `enabled` | `true` 默认开启，`false` 默认关闭；Forge 菜单里的开关只影响本次运行，不写回配置文件 |
| `font_size` | 720p 下的基础字号，允许 `8～128`，例如改为 `42.0` 即增大到 42 像素。1080p 按 1.5 倍绘制；命中最初 0.15 秒还有最多 4 像素的短暂放大 |
| `font_file` | `default` 使用 Forge 默认字体；填 `.ttf` 文件名可使用伤害数字专用字体，见下一节 |
| `highlight_color` | 肉质 ≥45 和无视物理肉质伤害的颜色，默认黄色；例如 `80, 255, 120` 改为绿色 |
| `normal_color` | 肉质 <45 的颜色，默认白色 |
| `outline_color` / `outline_width` | 描边颜色和 720p 下的宽度；宽度允许 `0～10`，`0` 关闭描边 |
| `full_opacity` / `lifetime` | 完全不透明秒数和总显示秒数；例如 `1.50` / `1.80` 在最后 0.30 秒淡出。保持 `0 < full_opacity < lifetime <= 60` |

颜色按 **红、绿、蓝（RGB）** 顺序填写三个用逗号分隔的整数，每项为 `0～255`，不加方括号或引号。透明度由淡出逻辑控制。修改颜色只改变外观，肉质的 45 分界及瓶伤分类保持原来的判定。

配置在插件初始化时读取一次。**改完重启游戏**；正在运行的游戏不会自动重载。编辑项目里的模板不会直接改变已安装的配置，用户应修改上面给出的实际文件。Switch 实机放在对应游戏 mod 的 `romfs/nativeNX/plugins/`，与 NRO 并排。

省略某个参数会沿用该项默认值。配置文件缺失、超出 4096 字节、不是 UTF-8 或存在填写错误时，整个配置使用默认值，并在 Forge 日志中记录文件路径、错误行号和原因。参数名拼错也会报告错误，避免用户误以为已经应用。支持 `;` 和 `#` 注释、Windows 换行及 UTF-8 BOM。

## 更换伤害数字字体

保持 `font_file = default` 时，本插件使用 Forge 当前默认字体。要单独换伤害数字的字形，把**有效 TTF 字体文件**放到：

```text
C:/Users/Lawrence/AppData/Roaming/Ryujinx/mods/contents/0100770008dd8000/romfs/nativeNX/forge/fonts/
```

例如字体文件名为 `MyFont.ttf`，将配置改为：

```ini
font_file = MyFont.ttf
```

只填文件名，大小写与实际文件一致，不加目录或引号；文件名需要以 `.ttf` 结尾。保存并重启游戏，无需重新编译，也不用覆盖 Forge 自带的 `Roboto-Medium.ttf`。本插件在 `forge_onImGuiInit` 中单独加载字体，在伤害绘制时切换、绘制完成后恢复，所以其他插件和 Forge 菜单的字体不变。指定文件找不到或字体加载返回失败时，日志会说明并继续使用 Forge 默认字体。

数字大小由本插件的 `font_size` 控制，Forge 的 `menu_font_size` 只用于其菜单。本次没有修改或替换用户的字体文件。

## 源码结构

源码只有两个 Rust 文件：

- [data.rs](./src/data.rs)：配置字段和 INI 解析、事件字段、颜色判定、玩家/区域标识、数字缓冲、64 条固定队列、共享状态、钩子存储，以及队列/数据操作。十九项主机测试直接放在文件末尾的 `#[cfg(test)]` 中。
- [lib.rs](./src/lib.rs)：Forge 入口、配置读取、专用字体初始化、命中/伤害钩子、本地玩家筛选、实时肉质读取、原函数调用、坐标投影、上浮淡出和设置界面。

读取顺序：`main` 安装两个钩子 → `collision` 保存本次命中 → `damage` 读取原入账结果并入队 → `forge_onUpdate` 投影 → `draw_damage` 绘制。游戏偏移和参数来源见下面的表。

主要数据字段：`State.config` 保存初始化读取的配置，绘制快照带有同一配置，事件淡出和过期均读取其中的时间。`Event.world` 是命中瞬间的世界 XYZ；`screen` 是归一化屏幕坐标；`age` 是已显示秒数；`yellow` 保存本次伤害是否使用高亮颜色（默认黄/白）；`digits` 是数字文本；`serial` 用于阻止旧投影覆盖新事件。`Epoch.player/room` 用于切区及玩家对象变化时清理；`Contact.monster/slot` 用于把入账伤害匹配回本次命中，`attack/part_index` 仅在本次原命中调用尚未返回时读取攻击类型和肉质。

配置只读，不写游戏存档；通过 `MtFile` 读取 `app:/nativeNX/plugins/attack_damage_display.ini`。文件检查直接调用当前 Forge 导出的 `forge_fs_fileExists`，避免 SDK 的 CString 包装引入当前加载器未导出的 `malloc/free`。解析使用固定缓冲区和原有 Rust 核心库，没有新增 INI 解析依赖；源码中的缺省值在 `Config::DEFAULT` 定义。

链接参数写在 `.cargo/config.toml`，构建脚本是 `build.sh`；没有另拆 `build.rs` 或测试 Rust 文件。

## 安装与操作

首次安装时，把 [attack_damage_display.nro](./attack_damage_display.nro) 和 [attack_damage_display.ini](./attack_damage_display.ini) 一起放入游戏 mod 的 `romfs/nativeNX/plugins/`，然后重新启动游戏。本机对应目录：

```text
C:/Users/Lawrence/AppData/Roaming/Ryujinx/mods/contents/0100770008dd8000/romfs/nativeNX/plugins/
```

v0.3 NRO 和配置文件已放入上述 Ryujinx 插件目录，NRO 的内容及 SHA-256 与项目产物一致。重启游戏后加载，本次没有启动或重启游戏。它是独立 NRO，可与现有五个 `cb_*.nro` 一起加载；不使用它们的钩子地址。

打开 Forge 菜单后，可在 `MHGU Attack Damage Display v0.3` 区域切换 `Show attack damage`。默认无需开菜单也会绘制。后续升级替换同名 NRO，并保留已经修改的 INI；插件目录只保留一个伤害显示插件副本。

- `Local contacts`：本地玩家进入命中处理的次数，包含最终没有入账的接触。
- `Damage events`：收到正数入账伤害并成功加入队列的次数。
- `Visible numbers`：最近一帧通过投影、正在绘制的数字数。

## 伤害和坐标来源

| 环节 | main 偏移与具体行为 |
| --- | --- |
| 命中上下文 | `00170968`，保留原八个参数；`point` 为世界 XYZ，`part+8` 为部位累加组 |
| 本地来源 | `attack+6D == 1`；`attack+10 == 0` 取 `attack+0C`，转交路径在 `attack+6E == 0` 时取 `attack+70`；与 `00276278(*(main+01885B58))` 返回的本地玩家比较 |
| 最终入账 | `0009D6F8(monster, slot, raw, mode)` 先执行原函数，再读取其有符号 16 位返回值；与当前命中的怪物和部位组匹配 |
| 实时肉质 | `000BAB60(monster, part_index)`，其中 `part_index=*(part+6)`，不同于 `part+8` 的部位累加组；返回数组的 `+0/+1/+2` 分别为斩/打/弹肉质 |
| 攻击分类 | `attack+30` 的 `0x20` 为斩/打权重分支，`0x10000` 为指定斩或打的分支，`0x8000` 为两者都传入 100 的分支，普通弹肉质分支使用 bit0；这些分支均不使用时物理倍率为 1，直接黄字 |
| 投影 | `00018A48(out4, camera_manager, world, view)`；相机来源 `main+0184D750`，视口索引 `*(main+0212668C)+EC`；原视口宽高归一化后映射到 Forge DisplaySize |
| 绘制 | `forge_onImGuiFreeRender`，无需显示 Forge 主窗口 |

这里显示的是游戏每次命中最终写入部位累加器的正数，包含原函数执行的怪物侧倍率、睡眠修正和整数化。怪物 HP 在后续更新中汇总扣除；最后一击的数字没有按怪物剩余 HP 截断。

本版过滤 `mode=2` 的 10000 特殊处理、`slot>=8`、零或负数返回值，以及没有本次命中上下文的调用。队友、随从、非猎人分类的炸弹、毒等持续扣血没有接入。普通武器和猎人归属的投射物/瓶爆通过相同来源字段匹配；各类实战行为尚待游戏验证。

斩/打权重读取玩家 `+2B70/+2B71`，比较“斩肉质×斩权重”和“打肉质×打权重”，按原物理肉质函数的选择结果取肉质值，再与 45 比较；权重乘积相等时取斩肉质。因此不会因为同一部位另一种攻击的肉质较高而错误变黄。`+13CC bit2` 的原结算路径只传入 `attack+40 bit1` 指定的一个权重；`0x10000` 分支也由该字段选择斩/打。普通射击直接取弹肉质。

依据为 main 的 `0016DC6C`（物理肉质选择）、`00178030/001748A8`（斩/打权重）、`001784D8/001785E8`（特殊物理分支）、`00178660`（弹肉质）以及 `00173A24/00176AD0`（最终伤害流程）。45 的分界亦见 `0017885C` 的弱点特效判定。这里使用实时表中的肉质值，不把斩味倍率、会心倍率或最终伤害数值当成肉质；属性部分继续完全由游戏原公式结算，黄字不表示属性肉质也达到 45。

玩家对象地址变化或玩家 `+1054` 区域号变化时清理旧事件。事件不保存怪物、部位指针供后续绘图使用。所有游戏指针访问及投影在命中调用或游戏更新阶段完成，绘图回调只读取插件自己的快照。

按本次要求，代码没有通用空指针检查：命中参数、本地玩家、相机管理器和 UI 管理器依赖原游戏的有效任务生命周期。钩子安装依赖 Forge 正常完成 JIT 分配；游戏更新后仍有事件时，相机和 UI 管理器应保持有效。任务退出、切区、暂停恢复是实战验证的一部分。

## 装载兼容性

安装前核对命中、伤害、投影、本地玩家及肉质读取函数入口的十个 ARM 指令字；不匹配就记录偏移和实值并跳过安装。两个实际修改的入口分别为 `00170968` 和 `0009D6F8`，前八字节没有 PC 相对寻址，可以直接搬入当前 Forge 跳板。

本机 `subsdk9` 的 `forge_hook_create` 实际返回结构大小为 `0x4C`（76 字节），从其 ARM 返回路径的 memcpy 长度确认。当前 Rust SDK 的 `forge_sys::hook::Hook` 缺少 context 字段，Jit 数组长度也与该加载器不同。本插件在 [lib.rs](./src/lib.rs) 中使用经核对的 76 字节不透明 C 返回类型调用同一 Forge API，保留两个跳板的返回对象。没有修改共享 SDK 或已有盾斧插件。

所有 18 个强导入已在本机实际 subsdk9 导出表中确认，八个装载/绘制入口（包括专用字体初始化）均存在于最终 ELF 动态符号表。与其它 Forge 版本的结构兼容性没有验证。

## 构建与离线验证

从本目录运行：

```bash
bash "./build.sh"
bash "./test.sh"
```

使用本机 nightly Rust、devkitARM、已有 Forge SDK 和 Cargo 缓存，构建过程关闭 Cargo 网络访问。目标配置与链接脚本复用 `../sample-plugin/`；链接参数由 `.cargo/config.toml` 提供，`build.sh` 会先切到本项目目录。Cargo 包名保持 `mhgu-attack-damage-display`，ELF 文件为 `target/armv7a-none-eabi/release/libmhgu_attack_damage_display.so`，最终 NRO 为 `attack_damage_display.nro`。

十九项主机测试覆盖：原有斩/打/弹 44/45 分界、混合权重、特殊物理分支、固定瓶伤、默认淡出、十瓶坐标、64 条容量覆盖、旧投影保护、切区清理、过期、视口归一化、非法伤害及四线程并发；新增配套 INI 与缺省值一致、UTF-8 BOM/Windows 换行/注释/专用字体、缺省字段及错误行号、非法数值和 RGB、字体文件名范围，以及配置时间实际控制不透明时段、淡出及过期。测试不执行游戏代码。

[verify-build.cjs](./tools/verify-build.cjs) 对照 ELF/NRO 段内容、ARM32 soft-float ABI、MOD0/动态表/BSS、回调导出、当前加载器导入和 Hook 结构大小。结果记录在 `target/build-verification.json`。

本版已经通过编译、十九项主机测试和二进制装载条件检查。**游戏内配置读取、专用字体加载及实际显示效果尚未实测。** 重启后检查 Forge 日志中 `loaded app:/nativeNX/plugins/attack_damage_display.ini`，然后可把 `font_size` 改为 42 并重启，对照字号变化。肉质规则仍可用已知斩肉质分别为 44 和 45 的部位对照，随后在硬部位测试榴弹瓶，再测试连续超解、切区和返回村庄；通过菜单计数区分“没有采到命中”和“已有事件但没有可见投影”。

按数据和逻辑两个文件组织，固定容量且每次命中不分配堆内存；复用原伤害结算及投影，各个函数直接对应采集、入账、投影和绘图步骤。
