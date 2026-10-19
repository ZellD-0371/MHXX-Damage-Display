# MHGU 显伤数字偏右下的分析

分析日期：2026-10-07。目标是现有 `attack_damage_display.nro` 的位置问题。

用户已在手机 yuzu、1×分辨率、掌机模式复现，并进一步确认主机模式位置正确。**已定位并修正旧版将交换纹理尺寸当作可见画面尺寸的换算错误，编译并部署 v0.4。** 22 项主机测试和当前 Forge 的二进制装载检查通过；修复后手机画面仍待用户实测。

## 已定位的原因和修复

GU 图形管理器为 `*(main+0x02126998)`。其 `+0xB0/+0xB4` 保存当前可见画面宽高；原函数 `main+0x00BBBDE0` 初始化时、`main+0x00BC0874` 切换尺寸时，将这两个字段传给 `main+0x0086A24C`，最终调用 `nvnWindowSetCrop(window, 0, 0, width, height)`。ARM 指令也确认 `main+0x00BC0B80` 从 `+0xB0` 连读两个整数，`main+0x00BC0B90` 调用裁剪包装函数。裁剪原点固定为 `(0,0)`。

图形管理器构造函数 `main+0x00BBCA50` 中，交换纹理的默认分配尺寸为 `1920×1080`，掌机显示尺寸为 `1280×720`。交换纹理创建使用最大尺寸，可见裁剪使用当前尺寸。`main+0x00BBE154` 收到操作/性能模式变更后更新当前宽高，随后更新裁剪。Forge 后端源码 `ImGui_ImplNVN_NewFrame` 从交换纹理 `GetWidth()/GetHeight()` 设置 `ImGuiIO.DisplaySize`，绘图时以整张纹理建立正交投影和 GPU viewport。

v0.3 将归一化命中坐标乘以 `DisplaySize`：掌机可见中心应为 `(640,360)`，按整张 `1920×1080` 计算却得到 `(960,540)`，坐标放大为 1.5 倍，符合偏右下和屏幕外的表现。主机显示全张纹理时两者一致，因此位置正常。这是插件的换算问题，不需要归因于用户操作或其它金手指。

v0.4 每次游戏更新后读取图形管理器 `+0xB0/+0xB4`，随本次投影存入插件快照。绘图仅用快照中的可见宽高计算锚点，字号、描边、横向错开和上浮距离按可见高度与 720 的比值缩放。保留原游戏投影、相机选择及两个原钩子；没有增加配置项、绘图依赖或第三个 Rust 文件。

源码：[画面宽高读取](D:/Work/MHGU-forge/mhgu-attack-damage-display/src/lib.rs:267)、[快照与绘图](D:/Work/MHGU-forge/mhgu-attack-damage-display/src/lib.rs:134)、[数据与回归测试](D:/Work/MHGU-forge/mhgu-attack-damage-display/src/data.rs:261)。原版证据见 [裁剪函数](D:/Work/MHGU-forge/mhgu-attack-damage-display/work/20261006-damage-projection/evidence/main-86A24C.c)、[模式更新](D:/Work/MHGU-forge/mhgu-attack-damage-display/work/20261006-damage-projection/evidence/main-BBE154.c) 和 [ARM 调用链](D:/Work/MHGU-forge/mhgu-attack-damage-display/work/20261006-damage-projection/evidence/window-crop-call.asm)。

## 已确认的行为

1. 数字保存命中瞬间的世界坐标，后续只随相机重新投影。事件不保存怪物部位或骨骼，所以不会随怪物身体移动。
2. 碰撞入口 `main+0x170968` 的第 4 个参数是接触点。上游球体碰撞函数生成接触点，下游 `main+0x173A24` 将其 XYZ 写到攻击对象的 `+0x20/+0x24/+0x28`。当前读取参数顺序与原函数一致。
3. 投影入口 `main+0x18A48` 的四个参数依次是输出、相机管理器、世界坐标、相机槽位。相机记录间距为 `0x1B0`，矩形在记录的 `+0x68/+0x6C/+0x70/+0x74`。插件使用的参数和偏移与原函数一致。
4. 原投影函数已完成透视除法和 Y 轴方向转换。返回的第 4 个浮点数表示前后方向；ARM 指令确认有效前方结果为正。不能再补一次透视除法或直接翻转 Y。
5. 插件用 Forge 的 foreground draw list 画字，没有调用道具栏的血量文本控件。落在道具栏附近是位置结果，不能据此判断它绑定了道具栏。
6. 插件先判断锚点在屏幕内，再减去半段文字宽度、加入横向错开和上浮。没有对最终文字矩形重新限制，因此贴边数字会被裁掉；这不能单独解释持续偏到右下的现象。

源码证据：[命中点读取和投影](D:/Work/MHGU-forge/mhgu-attack-damage-display/src/lib.rs)、[事件字段](D:/Work/MHGU-forge/mhgu-attack-damage-display/src/data.rs:223)。

## 归一化坐标为何仍然会偏移

旧链路是：原函数返回视口内坐标 → 除以游戏视口宽高 → 乘以 ImGui 的 `DisplaySize` → Forge 绘制。新链路在最后一次乘法使用游戏的当前可见裁剪宽高。

原函数的横坐标为 `(NDC.x + 1) / 2 * 游戏视口宽度`；除以同一个宽度后，宽度抵消。因此，**当使用的相机正确、游戏视口覆盖可见画面、ImGui 尺寸与其绘图范围一致时，720p 和 1080p 会得到同一个相对位置。** 仅把输出尺寸换大不会产生偏移。

较大的交换纹理包含可见裁剪范围，但 ImGui 不自动按 NVN Window 的 crop 改变 `DisplaySize`。本次逆向确认了两种尺寸的创建和传递字段，因此修复直接读取裁剪宽高。上面的中心坐标也作为离线回归用例；没有将其描述成手机内存实测值。

截图的手机宽度、两侧黑边和触摸按钮属于 Android 最终显示布局。NRO 在游戏的 NVN 纹理上绘制，不能直接把截图宽度或黑边宽度填进插件。

本机已安装 Forge 的二进制包含 ImGui `1.92.8`，与 Rust 绑定版本一致；检查了 `ImDrawList_AddText_FontPtr` 的 ARM 参数传递。本机版本的检查结果不能替代手机上实际前置版本的检查。`DisplaySize` 是绘图使用的像素尺寸，详见 [ImGui 1.92.8 原始定义](https://github.com/ocornut/imgui/blob/v1.92.8/imgui.h#L2217)。

## 验证结果与手机复测

| 环节 | v0.4 实现 | 已完成的验证 |
| --- | --- | --- |
| 游戏投影 | 更新结束后读取 GUI 对象 `+0xEC` 作为相机槽位 | 原参数、视口归一化及前后方向检查；相机选择没有改动 |
| Forge 绘图 | 乘游戏当前可见裁剪宽高后在 foreground 画字 | 720p 裁剪中中心/右下定位、1080p 定位与字号，以及掌机→主机→掌机的已有事件重映射 |

原游戏创建主相机时将其写入管理器 `+0x54`，对应槽位 0。原 GUI 投影辅助函数也读取 GUI `+0xEC`；但目前没有手机采样，不能宣称本次读取到了错误槽位。直接强制槽位 0属于需要对照测试的改动。

已通过 release 构建、22 项主机测试、ARM32 soft-float ABI、ELF/NRO 段一致性、MOD0/动态表/BSS、8 个回调、18 个强导入及 76 字节 Hook 返回结构检查。新 NRO 为 57344 字节，SHA-256：`a61c303faed1db52e14212a8457e03025eb9044cd352f2ad8dfff0838e07056b`。

已备份旧 NRO 到 `legacy-builds/20261007-before-handheld-fix/`，替换本机 Ryujinx mod 中的 `attack_damage_display.nro`，并逐个比对确认用户 INI、其它插件及 `subsdk9` 内容保持原样。手机安装时仅替换自己游戏 mod 中的同名 NRO，保留 INI，完全退出并重启游戏，在 1× 掌机模式攻击同一部位检查数字位置。具体显示效果和切换时机需要手机实战确认。

## Evidence → Finding → Path

范围：[scope.md](D:/Work/MHGU-forge/mhgu-attack-damage-display/work/20261006-damage-projection/scope.md)。用户后续明确要求修复；范围已补入显伤源码、离线构建和测试，以及备份并替换本机显伤 NRO。

| Evidence | source_ref | 复核方法 | content_hash |
| --- | --- | --- | --- |
| E-01 | 修复前 `src/lib.rs`、`src/data.rs` | 修复前证据清单，v0.3 代码可从项目 Git 历史只读查看 | 见原证据清单 |
| E-02 | 原版 `main.c`：`18A48`、`551340`、`167118`、`173A24` | 下方 Node 命令；提取文件在 case/evidence | 见证据清单 |
| E-03 | `projection-tail.bin` 的 ARM 指令 | 下方 objdump 命令 | 见证据清单 |
| E-04 | 本机 `subsdk9` 与已安装 NRO | NSO 解压、动态符号及 ARM 参数检查 | Forge：`724247b3a8b4c478bf2a7966b492e7920dcc18d7f2aa2b2f9571a3ce99a5718b`；NRO：`034c9da8a02d6e6bb64559fcbf1ae0b77ae16cd8e96a15cf652d732463ce79b5` |
| E-05 | 用户截图及“1×，掌机模式”回复 | 原图视觉检查、用户复现信息 | 截图见证据清单；回复 n/a |
| E-06 | 原版构造、模式更新及 NVN Window 裁剪链；用户掌机错误/主机正常对照 | `main-BBCA50.c`、`main-BBE154.c`、`main-BC0874.c` 和两段 ARM 指令；对照 Forge 后端源码 | `evidence/crop-evidence-hashes.json` |
| E-07 | v0.4 源码、构建和部署结果 | `bash build.sh`、`bash test.sh`；产物、备份及其它插件逐个 SHA-256 比对 | `evidence/v0.4-build-deploy.json` |

| Finding | severity | evidence_ids | confidence | location | status |
| --- | --- | --- | --- | --- | --- |
| F-01 数字不随怪物身体移动 | n/a_re | E-01 | 高 | Event.world | 已确认 |
| F-02 参数与基础投影换算一致 | n/a_re | E-01、E-02、E-03 | 高 | Projection.project | 已确认静态一致 |
| F-03 贴边文字可被裁掉 | n/a_re | E-01 | 高 | draw_damage | 已确认代码行为 |
| F-04 手机 1×掌机有位置偏差，主机正常 | n/a_re | E-05、E-06 | 高 | 手机 yuzu | 用户复现 |
| F-05 旧版把最大交换纹理当作可见显示范围 | n/a_re | E-01、E-06、E-07 | 高 | draw_damage 的坐标和外观缩放 | v0.4 已修正；离线验证通过，手机复测待完成 |

P-01，path_type=callflow：碰撞接触点（E-02）→ collision 复制 XYZ → damage 保存数字 → forge_onUpdate 调原函数投影并读取当前裁剪宽高（E-06）→ commit_projection 保存屏幕坐标和宽高 → draw_damage 按可见画面计算文字位置并调用 Forge（E-07）。

在 `D:/Work/MHGU-forge` 下复核原版函数：

```bash
node ".codex-tmp/ui-damage-analysis/inspect.js" functions 18A48 551340 167118 173A24
node ".codex-tmp/ui-damage-analysis/inspect.js" functions 86A24C BBBDE0 BBCA50 BBE154 BC0874
"C:/devkitPro/devkitARM/bin/arm-none-eabi-objdump.exe" -D -b binary -m arm --adjust-vma=0x18C60 "mhgu-attack-damage-display/work/20261006-damage-projection/evidence/projection-tail.bin"
```

时间线：[timeline.md](D:/Work/MHGU-forge/mhgu-attack-damage-display/work/20261006-damage-projection/timeline.md)。本次已完成代码修复、构建、离线检查和本机 NRO 替换；没有连接手机运行时，也没有宣称完成修复后的手机实战验证。
