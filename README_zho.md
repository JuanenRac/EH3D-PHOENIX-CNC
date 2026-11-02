<p align="center">
  <img src="images/EH3D_BANNER.svg" alt="EH3D-MOTIONCNC 横幅" width="100%">
</p>

# 🧭 EH3D-MOTIONCNC

<p align="center"><a href="README.md">🇺🇸 English</a> | <a href="README_spa.md">🇪🇸 Español</a> | <a href="README_fra.md">🇫🇷 Français</a> | <a href="README_ita.md">🇮🇹 Italiano</a> | <a href="README_deu.md">🇩🇪 Deutsch</a> | 🇨🇳 <b>中文</b> | <a href="README_jpn.md">🇯🇵 日本語</a> | <a href="README_por.md">🇵🇹 Português</a></p>

### EH3D 机床控制程序的共享基础：作业视图、主题以及八种语言的界面文本

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.0.8-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-scaffolding-FFB020.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-72-0f6cbd.svg" alt="Tests">
</p>

---

**诚实性检查 - 今天实际能运行的内容：**一个带窗口的 Rust 程序（`motioncnc`），驱动一台**模拟**机床：显示机床位置（工件和机床坐标），用点动面板手动移动，设置零点，回零，接收操作员输入的命令，逐行运行 G 代码程序（可暂停、停止和急停），在工作台上画出程序路径并标出已完成的行，并在控制器拒绝处停下。窗口支持 37 种语言和十一种主题。控制器接口（`eh3d-controller`，在 EH3D-COMMON 中）针对脚本化的线路编写了 GRBL，其余系列列为尚未支持。72 个测试在计算机上运行。**这里没有任何内容在真实机床上运行过**，仅在使用 GRBL 时打开串口，仅在向服务器发送程序时建立网络连接，模拟器既不绘制也不执行圆弧和其他代码。

---

> **安全状态：**本程序只驱动模拟机床：不打开串口或网络连接。连接真实控制器时，必须先在模拟器上、再在真实机床上证明可用，才能称为可用；限位开关和急停仍由机床自己的控制单元负责，窗口里的急停不是安全回路。

## 📋 速览

| | |
|---|---|
| 版本 | `0.0.8` |
| 成熟度 | 脚手架 |
| 测试 | 72 |
| 技术栈 | Rust |
| 真实硬件 | 否：只驱动模拟机床 |

**跳转到：** [概述](#-概述) · [架构](#-架构) · [快速开始](#-快速开始) · [测试](#-测试) · [结构](#-仓库结构) · [相关项目](#-相关项目)

## 🚦 项目状态

每一行是本仓库的一个部分，灯号表示它今天做到了哪一步。内容依据仓库能够展示的证据（测试、构建、运行），而不是计划。

🟢 已完成并验证 · 🟡 可用，有已知限制 · 🟠 部分完成或尚未验证 · 🔴 未完成或不可用

| 领域 | 状态 | 在这里的含义 |
|---|---|---|
| 基础库 | 🟡 | 一个 Rust crate，以本程序的名称重新导出 `eh3d-view` 的 `scene`、`texts` 和 `theme`；需要旁边有 EH3D-COMMON 文件夹。 |
| 场景与视图 | 🟡 | 在 EH3D-COMMON 中编写并测试；本程序的窗口在工作台上绘制程序路径和刀具位置（已在电脑上看过）。 |
| 主题与文本 | 🟡 | 十一种主题和 37 种语言的界面文字，已在本程序的窗口中看过。 |
| 测试 | 🟡 | 电脑上通过 72 个测试（`cargo test --features gui`）：会话（点动、零点、回零、程序、暂停、报警、急停）、程序路径和命令行；模拟控制器在 EH3D-COMMON 中测试。 |
| 窗口与点动 | 🟡 | 在电脑上配合模拟控制器运行：位置、点动面板、步长、进给、回零、清零、解锁、命令行和日志；没有真实机床。 |
| 机床控制 | 🟠 | 只有模拟器：程序逐行运行，可暂停、停止和急停；这里还没有连接任何真实控制器。 |
| 文档 | 🟡 | 八种语言的 README、两张图片、三个示例程序和更新日志。 |

## 🎯 概述

* **窗口**（`src/gui.rs`，`motioncnc`）：机床位置（工件和机床坐标）、带步长和进给的点动面板、回零、清零和解锁、急停、带日志的命令行，以及在工作台上显示路径的程序。
* **会话**（`src/session.rs`）：程序对 `eh3d-controller` 中任何控制器所做的事：点动、清零、回零、发送一行、逐行运行程序、暂停、停止和报警；在模拟控制器上测试。
* **程序路径**（`src/gcode.rs`）：直线移动（`G0`、`G1`，绝对或相对，毫米或英寸），在程序运行前绘制；圆弧不绘制。
* **模拟控制器**（`eh3d_controller::sim`，在 EH3D-COMMON 中）：内存中的机床，超出行程会报警；市场上的控制器系列列在 `eh3d-controller` 中，GRBL 是第一个编写的，其余显示为尚未支持。
* `eh3d-view` 的**主题和文字**：Forge（默认）、Neon Arcade、Glass Pro、经典 Windows 灰色和一个跟随 Windows 的主题，以及英语、西班牙语、意大利语、德语、法语、葡萄牙语、中文和日语界面。它将成为：基于 EH3D-DRILLPRO 和 EH3D-ROUTEPRO 引擎的、用于钻孔、铣削以及 2D 和 3D CNC 加工的一个程序；这部分尚不存在。
* **菜单、设置和文件**：文件、视图、程序、机床和帮助菜单，包含 PlanetCNC 中有的功能（打开、最近文件、保存、编辑、各零点、已存位置、主轴和冷却液）；两次运行之间会记住的设置窗口（单位、语言、十一种主题、行程限位、点动）；以及套件共用的打开和保存窗口（`eh3d-filedialog`：位置、排序、过滤、保存文件的扩展名和替换前的询问）。
* **串口上的 GRBL：** 使用 `--features serial` 构建后，`motioncnc --grbl 端口[@波特率]` 驱动 GRBL 控制器（默认 115200 波特）而不是模拟器，`motioncnc --ports` 列出串口；没有该功能的构建或打不开的端口会给出提示并退出。只对脚本化的控制器试过，从未对真实控制器试过。
* **随屏幕变化的窗口：** 窗口以最大化打开，从 720p 到 1080p 乃至更大更宽的屏幕，所有内容随之放大或缩小。
* **故障后继续：** 程序运行时，已到达的行会保存到一个小文件中；如果窗口、电脑或连接中断，下次启动时会询问是从该行继续还是放弃。操作员回答之前不会向控制器发送任何内容。
* **发送到 EH3D 服务器：** 在设置中填写服务器地址、本程序的令牌和机器 ID（设置窗口的“服务器”部分，或设置文件中的 `server_address`、`server_token`、`server_machine`）后，**程序 > 发送到服务器** 会把程序文本作为 G 代码作业发送；服务器在它的机器上运行（其串口上的 GRBL 控制器，或它的模拟器），窗口跟踪作业：状态、百分比和已到达的行显示在顶部栏、面板和日志中。窗口的开始、暂停和停止会传到服务器上的作业，关闭窗口不会停止它；已停止的作业从服务器到达的行继续（窗口下次启动时会提示，与控制器上的程序相同）：服务器会恢复生效的代码，并在第一次移动前等待操作员。只有设置 `server_restore_outputs = yes` 时才会重新启动主轴和冷却。已用替身服务器试过；服务器自己的测试会向其模拟器发送相同的请求。从未在实机上试过。
* **故障后继续：** 中途停下的程序会先恢复到原来的状态：单位和绝对距离、安全 Z（设置文件中的 `resume_safe_z`，5 mm）、主轴及其达到转速所需的时间（`resume_spindle_wait`，3 秒）、冷却液、已到达的位置和模态状态；恢复文件保存程序的 SHA-256，程序改变后不会继续。
* **窗口的辅助功能：**程序开始前的**摘要**（移动次数、切削距离、按设置中的速度估算的时间、各轴的行程范围，以及与机器行程的核对，并带有套件共用的警告：程序过小或离角落太远、超过工作台、估算未计入的圆弧、在设定进给之前的切削；**仍然开始**，或在程序超出行程时拒绝，`reject_out_of_travel = no` 可将拒绝改为警告，`confirm_start = no` 关闭摘要）；**键盘快捷键**（Ctrl+O 打开、Ctrl+S 保存、Ctrl+E 编辑、F5 开始、F6 暂停、F8 停止、Esc 关闭窗口、F1 列出全部；`shortcuts = no` 关闭）；带有绘制图标和名称的**工具栏**；用减号和加号调整或**直接输入**的数值；路径可在俯视图中**拖动和缩放**，或在 3D 中**旋转和缩放**并显示刀具；打开窗口中的 **G 代码文件预览**；**崩溃记录**（`eh3d-crash.log`）；以及设置中的安全部分（设置文件中的 `rapid`、`summary_tiny`、`summary_far` 和 `summary_margin`）。

## 🧩 在 EH3D 中的角色

EH3D-MOTIONCNC 是 EH3D 的机床控制程序，目前运行在模拟机床上。它通过路径依赖 EH3D-COMMON（文件夹 `EH3D-COMMON` 必须与本文件夹相邻：视图、主题、文字和控制器），目前不依赖任何其他 EH3D 仓库。

## 🏛️ 架构

<p align="center">
  <img src="images/architecture.svg" alt="EH3D-MOTIONCNC 架构：谁将使用它、包含什么、依赖什么" width="100%">
</p>

各部分如何配合：谁将使用它、本仓库包含什么、它依赖什么。图片由 `docs/architecture.json` 绘制，并与代码核对，未来的内容标为未来。

## 🖼️ 屏幕截图

<p align="center">
  <img src="images/screenshots/program-running.png" alt="a program running" width="100%">
</p>

<p align="center"><em>程序在模拟机床上运行：已完成的行为绿色，当前行高亮，路径绘制在工作台上，刀具位于机床所在位置。</em></p>

<p align="center">
  <img src="images/screenshots/spiral-neon.png" alt="Midnight theme" width="100%">
</p>

<p align="center"><em>午夜主题，一个已运行完的长程序。</em></p>

<p align="center">
  <img src="images/screenshots/square-german.png" alt="German interface, light theme" width="100%">
</p>

<p align="center"><em>使用浅色纸张主题的德语界面：窗口中的每个词都跟随语言。</em></p>

<p align="center">
  <img src="images/screenshots/settings-window.png" alt="settings window" width="100%">
</p>

<p align="center"><em>设置窗口，左侧的分区如同 PlanetCNC 的目录树：语言、十一种主题和显示内容。</em></p>

<p align="center">
  <img src="images/screenshots/open-window.png" alt="Open window" width="100%">
</p>

<p align="center"><em>套件共用的打开窗口：位置、最近文件、带日期和大小的文件夹列表、文件名和类型。</em></p>

## 🔒 安全与防护

本程序只有在为 GRBL 启动时（`--grbl`，用 `--features serial` 构建）才打开串口，只有在操作员向服务器发送程序时才连接设置中地址所指的 EH3D 服务器；令牌以文本形式保存在设置文件中。它不读取他人的文件，只读取操作员选择的 G 代码文件（作为文本读取，绝不自行运行），也不是安全系统。见 [SECURITY.md](SECURITY.md)。

## 🚀 快速开始

需要稳定版 Rust 工具链，以及本文件夹旁边的 `EH3D-COMMON` 文件夹。

```bash
cargo build --release --features gui
cargo test --features gui
cargo clippy --all-targets --features gui -- -D warnings
cargo fmt --all --check
python -I tools/ci_validate.py
cargo run --release --features gui -- examples/02_frame_and_pocket.nc
```

`build.bat` / `build.sh` 构建程序，`test.bat` / `test.sh` 运行测试，`run.bat` / `run.sh` 用示例程序打开窗口。除非设置了 `NOPAUSE`，脚本结束时会等待按键。

## 🧪 测试

`cargo test --features gui` 运行 72 个测试：会话（点动和清零、逐行运行程序、暂停和继续、被拒绝的行、急停和报警、命令日志、回零）、程序路径和命令行；视图和模拟控制器在 EH3D-COMMON 中测试。CI 还会运行 `cargo fmt --check` 和把警告当作错误的 `cargo clippy`，`python -I tools/ci_validate.py` 检查清单、版本、更新日志和八个 README。

## 🏗️ 构建与发布

版本号位于 `Cargo.toml` 和 `eh3d.project.json`，每个版本在 [CHANGELOG.md](CHANGELOG.md) 中有对应条目。目前还没有可打包的程序，所以没有现成的二进制文件。

## 📂 仓库结构

<p align="center">
  <img src="images/repository-structure.svg" alt="EH3D-MOTIONCNC 仓库结构" width="100%">
</p>

<details>
<summary>目录树的文本版</summary>

```text
EH3D-MOTIONCNC/
├── src/
│   ├── lib.rs                                   各模块，以及以本程序名称重新导出的 eh3d-view 视图
│   ├── gui.rs                                   窗口：位置、点动、程序、路径、命令和日志
│   ├── session.rs                               对控制器所做的事：点动、清零、回零、逐行运行程序、暂停、停止、报警
│   ├── gcode.rs                                 G 代码程序的直线移动，用于绘制其路径
│   ├── view.rs                                  可拖动缩放的平面图和可旋转的 3D 视图（仅数值）
│   ├── plan.rs                                  开始前的检查：移动的时间、范围和行程
│   ├── cli.rs                                   命令行：文件、语言、主题和截图选项
│   ├── bin/motioncnc.rs                         打开窗口的程序
│   ├── server.rs                                与 EH3D 服务器的连接：发送、跟踪、暂停、停止
│   ├── settings.rs                              两次运行间记住的设置
│   ├── gui/dialogs.rs                           菜单、设置、打开、保存、编辑器
│   ├── gui/remote.rs                            服务器上的程序：开始、暂停、停止与继续
│   ├── gui/views.rs                             窗口中央：俯视或 3D 的路径与刀具
│   ├── gui/summary.rs                           开始前的摘要窗口和按键列表
│   ├── gui/numfield.rs                          用减号和加号按钮或直接输入设置的数值
│   ├── gui/keys.rs                              键盘按键以及菜单和按键列表所读取的表
│   └── gui/icons.rs                             用线条绘制的按钮图标和工具栏按钮
├── examples/                                    三个示例 G 代码程序，从正方形到长螺旋
├── tools/                                       CI 检查、README 检查和图片工具
├── docs/                                        架构图和仓库结构图的源文件
├── .github/                                     CI 工作流、Issue 和 PR 模板
├── images/                                      横幅图、两张图片和屏幕截图
├── build.sh build.bat                           构建库
├── run.sh run.bat                               提示尚无程序并运行测试
├── eh3d.project.json                            清单：版本、角色、成熟度和构建命令
├── Cargo.toml Cargo.lock                        Rust 包及其锁定的依赖
├── CHANGELOG.md                                 每个版本的变更
├── SECURITY.md SUPPORT.md                       如何报告漏洞及在哪里提问
└── CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE   参与贡献、行为准则和许可证
```

</details>

## 🗺️ 路线图与已知限制

**目前已知的限制**

* 这里没有任何内容在真实机床上运行过：控制器是模拟器（或串口上的 GRBL，从未试过），只有测试会连接服务器。
* 模拟器只接受基本 G 代码（`G0`、`G1`、`G90`、`G91`、`G20`、`G21`、`G92`、`M3` 至 `M9`）；圆弧、固定循环和探测会被拒绝，程序路径也不绘制圆弧。
* 需要相邻的 `EH3D-COMMON` 文件夹，因为它通过路径依赖该文件夹。

**下一步**

* 真实控制器：先是通过串口的 GRBL，然后是其他系列，再然后是 HYDRA-UMC 的控制器。
* 模拟器和路径中的圆弧及其余 G 代码。
* 带切削高度的路径 3D 视图。
* 探测、换刀器和机床的材料。
* 合并 EH3D-DRILLPRO 和 EH3D-ROUTEPRO 的引擎，使一个程序既能钻孔又能铣削。

## 🌍 语言

README 和文本提供英语、西班牙语、法语、意大利语、德语、中文、日语和欧洲葡萄牙语版本。

## 🔗 相关项目

* [EH3D-COMMON](https://github.com/JuanenRac/EH3D-COMMON)：视图（`eh3d-view`）和作业 ZIP（`eh3d-archive`）所在的仓库
* [EH3D-DRILLPRO](https://github.com/JuanenRac/EH3D-DRILLPRO)：其引擎将合并到这里的钻孔程序
* [EH3D-ROUTEPRO](https://github.com/JuanenRac/EH3D-ROUTEPRO)：其引擎将合并到这里的铣削程序

## 📚 文档

* [变更日志](CHANGELOG.md)
* [安全政策](SECURITY.md)和[支持](SUPPORT.md)
* 作业视图的文档在它所在的 EH3D-COMMON 中。

## 🤝 参与贡献与社区

* 提交 pull request 之前请运行 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 和 `python -I tools/ci_validate.py`：全部必须通过。参见 [CONTRIBUTING.md](CONTRIBUTING.md) 和[行为准则](CODE_OF_CONDUCT.md)。
* README 有八种语言：修改文字时八个文件都要改，两幅图由 `tools/make_architecture.py` 和 `tools/make_structure.py` 根据 `docs/architecture.json` 和 `docs/structure.json` 绘制。
* 不要把没有在机床上运行过的内容描述为可用。
* 请私下报告漏洞：[SECURITY.md](SECURITY.md)。问题、想法和报告：electrohobby3d@gmail.com（[SUPPORT.md](SUPPORT.md)）。

## 👤 作者

**JuanenRac (Electro Hobby 3D)** · electrohobby3d@gmail.com

## 📜 许可证

软件：GPL-3.0-or-later。参见 [LICENSE](LICENSE)。套件的硬件设计使用 CERN OHL-S。
