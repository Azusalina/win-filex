# win-filex

## What is it?

filex 的 Windows 11 原生 Rust TUI 版本，与 `../filex` 并列维护。横向连接目录树，支持目录／直属文件分组树、搜索、预览、创建、重命名、复制、移动和回收站删除。CMD、PowerShell 与 Git Bash 可用同一个 `.exe`。

Windows 接口、限制和验证记录见 [description.md](description.md)。

## How to use

建议在 Windows Terminal 中使用 CMD、Windows PowerShell 5.1／PowerShell 7 或 Git Bash。构建需要 Rust 1.88+（推荐 rustup 的 MSVC 工具链）以及 Visual Studio C++ Build Tools；使用已构建的 `bin/filex.exe` 无需安装 Rust、Python、Node.js 或 WSL。

已提供交叉编译的 x64 `bin/filex.exe` 和 `dist/win-filex-windows-x64.zip`。将整个文件夹／压缩包复制到 Windows 解压后，可先运行 `filex.cmd` 或 `filex.ps1`；无需重新构建。安装预构建版使用 `scripts\install.ps1 -SkipBuild`。

需要在 Windows 自行构建时，在 PowerShell 中进入本文件夹：

```powershell
.\scripts\build.ps1 -Test
.\filex.ps1
# 可选：安装当前用户的 filex 命令，无需管理员权限
.\scripts\install.ps1 -SkipBuild
```

脚本执行策略阻止运行时，可以对这次脚本调用使用 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install.ps1 -SkipBuild`，无需永久修改系统策略。安装位置为 `%LOCALAPPDATA%\Programs\win-filex`，加入用户 PATH，并为 PowerShell 5.1／7 注册启动函数；`-NoProfile` 跳过 profile 注册。新终端中运行 `filex`，不传目录就以当前目录为根。未安装时：

| Shell | 启动 |
| --- | --- |
| PowerShell | `& 'C:\path\win-filex\filex.ps1'` |
| CMD | `C:\path\win-filex\filex.cmd`；批处理内使用 `call` |
| Git Bash | `source /c/path/win-filex/shell/filex.bash`，然后 `filex`；可将 source 行加入 `~/.bashrc` |

Git Bash 在 Windows Terminal 中直接运行；传统 mintty 优先自动使用随 Git for Windows 提供的 `winpty`。WSL Bash 请使用 `../filex` 的 Linux 版本。直接执行 `.exe` 也能浏览，但 `x` 会进入子 CMD；使用对应 shell 包装脚本，`x` 才能切换原终端目录。

- `↑/↓` 同层选择并首尾循环；`→` 展开／进入子级，`←` 返回父级；保持其他分支展开。`Enter` 展开目录或为文件选择打开方式。
- 默认同级垂直罗列；`v` 将直属文件分组／恢复普通树；O 排序，z 隐藏，Z 管理恢复。`Space` 标记多个文件／目录，显示亮洋红方框；`c/m` 后方向键选目标目录、Enter 执行。展开目录首位的 `+` 可选中按 Enter 直接放入待复制／移动的项目，`+` 键也可放入当前目标目录；`A` 新建目标子目录，创建后再 Enter 放入。Esc 取消待执行操作，再按一次清空标记。已有普通文件需再次 Enter 确认替换；目录／链接冲突或整批重名则拒绝操作。
- `Ctrl+F` 或 `/` 搜索；`.` 显示点文件及 Windows Hidden 属性项目。`h` 收起当前目录，`E/C` 展开／收起全部，`f` 当前目录设为根。
- `Ctrl+N`／`a` 新建文件，`Ctrl+Shift+N`／`A` 新建目录；`r` 重命名当前项目；输入框支持左右/Home/End 移动、在光标处插入与 Backspace/Delete。Windows Terminal 若拦截组合键，使用 `a/A` 或修改终端绑定。
- `Delete/D` 将当前项目或 Space 标记的整批项目移到 Windows 回收站，再按 `Enter` 确认，`Esc` 取消。`q` 信息面板，`[/]` 调整宽度；显示文件前几行、UTC 创建／修改时间、大小与链接信息。
- `y`／`Ctrl+Alt+C` 复制绝对路径到 Windows 剪贴板；`Ctrl+V` 在 filex 输入框粘贴内部已复制路径。`x` 退出至所选目录（选中文件使用父目录），`Q`／`Ctrl+C` 退出。
- `F5`／`Ctrl+F5`／`Ctrl+R` 刷新，`,` 重读配置，`?` 帮助。`--print-tree` 输出文本树，可重定向；`--ascii` 兼容缺少线框字形的字体。

主配置：`%APPDATA%\win-filex\filex.conf`。安装保留现有配置；无配置也能使用内置默认值。`filex --init-config` 创建示例，`filex --config 'C:\path\custom.conf'` 指定其他配置。TOML 可设置快捷键、文件扩展名／目录名颜色、排序、隐藏项、树缓存、分组树、面板宽度、预览行数及打开程序。Windows 路径建议使用 TOML 单引号，例如 `command = ['C:\Program Files\VSCodium\VSCodium.exe', '--', '{path}']`。

默认使用系统关联；`.py` 默认记事本。可在 `open_defaults` 为 `.py` 设 `python`、`.r` 设 `rscript`、`.sh` 设 `bash`、`.ps1` 设 `powershell`，脚本也可在 Enter 菜单中显式运行。对应程序需另行安装并加入 PATH。内置预览只读取 UTF-8 文本。

卸载：运行安装目录的 `scripts\uninstall.ps1`；默认保留配置，附加 `-RemoveConfig` 才移除配置。Git Bash 的 source 行由用户从 `~/.bashrc` 移除。便携使用无需系统安装，删除文件夹即可。

ZIP：在 `.zip` 上 Enter 选择 **Bash unzip / 解压 ZIP**（或 `o` 使用默认项），通过 Bash 调用 `unzip`，解压到压缩包旁全新的 `*.unpacked.随机后缀` 目录；原 ZIP 保留，不覆盖既有文件。需 Bash、unzip 和 Python 3（仅用于提取前校验 ZIP 路径／链接）；Windows 可用 Git Bash，并把 Python 加入 PATH。ZIP 内含越界路径或符号链接时会拒绝解压；失败保留部分输出目录以供检查。

多选方框默认为亮洋红、加粗显示，当前光标所在项继续反色；可在 `[colors]` 设置 `mark_color = "#FF55FF"`。TTY 使用 ASCII 方框，16 色终端映射到亮洋红。

打开菜单会隐藏不适用的专用程序：Python 对 `.py/.pyw`、Rscript 对 `.r`、Bash 对 `.sh/.bash`、unzip 对 `.zip`、Toolkit Markdown 对 `.md/.markdown`；Windows PowerShell 执行仅对 `.ps1` 显示。普通编辑器／系统关联仍通用。自定义 opener 可设置 `extensions = [".foo", ".bar"]`（大小写不敏感）；未设置或空列表表示通用。内置专用项即使旧配置没有 extensions 也会自动限制。

目录顺序可在配置中设置；置顶／沉底优先于“目录在前”，列表顺序也会保留：

```toml
[order]
top = ["index.html"]
bottom = ["node_modules", "__pycache__"]

[directory_order."~/Documents/site"]
top = ["index.html", "package.json"]
bottom = ["<other-folders>", "node_modules"]
```

`directory_order` 可用实际绝对目录路径（Windows 使用 TOML 单引号，例如 `[directory_order.'C:\work\site']`），或用目录名匹配所有同名目录；绝对路径规则优先，只作用于该目录直接子项。省略字段继承全局，显式 `[]` 清空继承。修改后按 `,` 重读配置。

Git 仓库展开后，树节点箭头附加状态标记并着色：`?` 未跟踪、`M` 未暂存修改、`S` 已暂存、`!` 冲突、`C` 已跟踪且干净但未配置上游、`P` 干净且与本地记录的上游一致、`↑/↓/↕` 分别为领先／落后／分叉（TTY 用 `^/v/D`）。文件名颜色保留，目录汇总子项状态，底部说明当前项目状态。颜色用于提示，字符标记在无色模式也保留。

Git 后台只读扫描，不自动 commit/push/fetch；`P` 不代表已实时查询服务器。F5／Ctrl+F5 更新本地快照；没有 Git 或查询失败时保持普通箭头。`[git] enabled = false` 可关闭，`max_entries` 默认 4000，每个窗口至多缓存 4 个已展开仓库；超出扫描范围不猜测状态。

排序列表中的 `"<other-folders>"` 匹配未在当前有效 top/bottom 中点名的所有直接子目录（不含目录软链接）。上述示例将 index.html、package.json 置顶，普通文件居中，其余子目录沉底，node_modules 最后；也可把该标记放入 top。显式名称优先，同组目录沿用 sort，隐藏目录需开启显示。

复制／移动：按 `c`／`m` 后可用光标选目标目录，或直接选目标文件进行替换；按 `t` 可输入目标地址，Enter 提交。单项支持目录或完整的新文件路径，多选要求已有目标目录；相对路径以源所在目录为基准，支持 `~`。已有普通文件会逐一提示，再按 Enter 确认覆盖，Esc 返回选择状态；全部覆盖确认完成后才开始本批操作。目录／链接冲突不覆盖。`g`／Ctrl+L 仍可输入目录并切换视图，再 Enter 执行。

选中文件夹后按 `!`，输入 `npm start` 等命令并 Enter，即以该目录为工作目录后台运行；Esc 取消输入。启动后显示 PID 和日志路径，关闭 filex 不停止命令。`[keys] run_background = "!"` 可改键。Linux 使用 `$SHELL -c`（默认 `/bin/sh`），日志在 `$XDG_STATE_HOME/filex/jobs` 或 `~/.local/state/filex/jobs`；Windows 使用 CMD，日志在 `%LOCALAPPDATA%\win-filex\jobs`。标准输入关闭，适用于不需要交互输入的命令；退出后仍可从日志看输出，依赖与 PATH 沿用启动 filex 的环境。

Linux 后台任务可用 `kill -- -PID` 停止对应进程组（PID 使用 filex 启动提示中的值）。日志需自行清理；服务能否启动仍取决于该项目依赖和命令自身。

`G` 打开当前选中项所属仓库的 Git 菜单：一键 `add ALL → commit → push`、只暂存当前文件／目录、提交全部已暂存内容、单独推送。菜单选择后输入提交信息（需要时），再 Enter 确认；Esc 取消。组合流程暂存整个仓库的新增／修改／删除，包含当前视图外的文件；只暂存当前项不受 Space 多选影响。执行时使用当前终端，支持认证和签名提示，完成或失败后 Enter 返回；失败即停止后续步骤，已完成步骤保留。不自动 pull、force push、设置上游或初始化仓库，push 遵循 Git 的 remote/refspec 配置；首次推送可先在终端设置 upstream。`[keys] git_menu = "G"` 可修改。

复制／移动模式下，目录首位的虚拟 `+` 是放入位置：选中它按 Enter，直接将待操作项目复制／移动到其所在目录；按 `+` 键也会放入当前目标目录（选中文件时使用其父目录）。`A`／配置的新建文件夹键仍可创建子目录，`t` 仍可输入目标地址。同名普通文件沿用 Enter 覆盖确认。


名称通配与完整顺序（文件和目录都适用，`%` 匹配任意长度，`-` 是普通字符）：

```toml
[directory_order."~/Documents/site"]
sequence = ["README.md", "d-%", "%-d", "%"]
# 首次匹配生效：README.md → d- 开头 → -d 结尾 → 所有剩余项。
# d% 匹配所有 d 开头的名称；=100%.txt 强制按字面匹配百分号。
```

`sequence` 优先于 top/bottom；未匹配项回退到原规则。top/bottom 同样支持 `%`，并保留显式名称高于通配的优先级；同一通配组内沿用原有 sort。分组树会分别在目录列、文件列体现该顺序，普通树可显示完整混合顺序。

`O` 编辑当前选中项的同级列表（根目录上则编辑其直接子项），↑↓ 选择，**K/J** 调整位置，Enter 保存为该目录专用 sequence；Esc 放弃修改。保存当前已加载、可见的项，并用末尾 `%` 接收未列出的项目；隐藏／未加载项不会被永久排除。保留配置其他设置与注释，并沿用配置软链接。

`z` 隐藏选中项，仅在 filex 的树和搜索中不可见，不重命名或删除；目录包含的整个子树同时隐藏。路径写入当前配置 `[browser] excluded_paths`，下次启动继续生效。`.` 只切换点文件，不能恢复这些排除项；`Z` 打开列表，选中项目 Enter 恢复。隐藏规则按绝对路径限定当前位置，不影响其他目录里的同名项；根目录本身不可隐藏。移动／重命名后按新路径处理，旧规则可在 Z 中移除。

Ctrl+F（或 `/`）输入关键词并 Enter 后进入搜索模式：↑／↓ 在匹配项目之间循环，可跨目录定位；n/N 仍可切换匹配。单个结果保持选中，零结果不移动。Esc 退出搜索模式，保留当前光标及展开状态；随后 ↑／↓ 恢复同级树导航。搜索模式中首次 Esc 不清空多选，也不取消待执行的复制／移动；再次 Esc 按原规则处理。

**F 临时聚焦**：选中文件／文件夹按 F，仅在该项的直接父目录内隐藏其他同级项，保留该项及其展开内容；父目录及更高祖先的其他分支照常显示和导航，↑↓ 跳过被隐藏项；各父目录可分别聚焦并叠加；在已聚焦项上再按 F，恢复该级并清除其全部子级聚焦，其他分支的聚焦保留。保留原根目录、展开状态、多选及排序，不修改 excluded_paths，不写入配置；退出重启即恢复。搜索跳过被隐藏的同级项及其子树，其他分支仍可查找，更换根目录清空聚焦，聚焦项被删除后清理对应记录。快捷键可用 `[keys] temporary_focus = "F"` 修改，小写 f 仍为设为根目录。

选中文件夹后按 **@（Shift+2）**，输入 `unzip archive.zip` 等命令并 Enter，在当前终端前台执行，显示输出和错误，支持覆盖提示等交互输入。命令结束后 Enter 返回并刷新树，Ctrl+C 可中断命令。`!` 仍在后台执行并写日志；两者都使用所选目录作为工作目录。`[keys] run_foreground = "@"` 可自定义。普通终端不能单独区分 ! 与 Shift+!，因此使用 @ 作为可兼容 TTY 的前台入口。

本次验证：两产品各 55 项共享 Rust 测试、80 项 Linux Python / PTY 检查全通过。临时聚焦覆盖 folder/file、自定义快捷键、保留父链与展开状态、原指针恢复、搜索范围、目标删除、根变更、批量展开收起不影响其他分支和传输期间恢复；前台命令覆盖当前目录、真实 unzip、读输入、输出/错误、非零退出、向 parent/child 发送 SIGINT 后返回界面并新建文件。最终 Linux release 的 8 项内存场景均小于 15 MB，最高 VmHWM 7.786 MB；原始记录及二进制 SHA256 见 fileX/memory-validation.json。Windows release 和测试程序已交叉编译，尚未实机验证。

2026-10-25 局部聚焦调整：F 仅隐藏所选项的直接同级及其子树，父级和更高祖先的其他分支保持显示、导航与搜索。两产品各 56 项 Rust 测试通过；Linux 81 项 Python / PTY 检查重跑全通过（首次一项窗口缩放时序失败，单项及整套重跑均通过）。最新 release 的 8 项内存场景峰值 7.309 MB；Windows release / 测试程序交叉编译通过，未实机验证。

2026-10-25 默认布局调整：内置值、示例配置及本机主配置使用 `[layout] grouped = false`，同级目录／文件默认垂直罗列；v 开启直属文件独立分组，再按恢复。F 仍只聚焦直接父目录内的分支。两产品各 56 项 Rust 测试及 Linux 26 项配置／导航／TTY 检查通过；Linux、Windows release 已重新编译，Windows 未实机验证。

2026-10-25 逐级聚焦：每个直接父目录分别保存聚焦项，多个层级和独立分支可同时生效；在已聚焦项上按 F 清除该项及所有后代的聚焦，保留其他分支。刷新分别清理失效记录，更换根目录清空全部，不写入配置。两产品各 57 项 Rust 测试及 4 项 Linux 实际 PTY 聚焦检查通过，覆盖嵌套、独立分支、父级解除联动、搜索、刷新、删除、传输和配置不变。Linux / Windows release 已构建，Windows 测试程序交叉编译通过，未实机验证。

2026-10-25 mv/cp 放入位置置顶：目标选择模式中，每个已展开目录的虚拟 + 固定为首项，刷新、新展开目录和分组树均保持；右键进入后可直接选中 +，Enter 放入该目录。退出传输模式移除虚拟项，实际文件排序不变。两产品各 57 项 Rust 测试通过，Linux 6 项实际 PTY 光标操作检查通过（含已有文件／子目录的目标目录、复制、移动、覆盖和取消）；Linux 与 Windows release 已构建，Windows 未实机验证。

## 独立仓库迁移（2026-10-27）

源码及本地数据迁至 `/home/a/Documents/products/filex-win`，已有提交历史保留。现在使用本项目自己的 `.git` 和 `origin` 独立提交、推送；不再通过 `someProducts` 的 push hook 发布。本次目录／仓库重命名保留配置目录、应用名称和已有用户数据。

2026-10-30：继承式目录排序


可用 `recursive_order` 为整棵目录树设置名称／扩展名顺序（包括该目录本身及任意深度的子目录）：

```toml
[recursive_order."~/Documents/sba"]
sequence = ["%.html", "%.css", "%.js", "%"]
```

`%` 是任意长度通配符：以上依次排列 HTML、CSS、JS，最后 `%` 接收剩余项；每组内部沿用 `sort`。也可混用名称，例如 `["README.md", "index.html", "%.html", "%.css", "%.js", "%"]`。Linux 名称匹配区分大小写。每个字段分别继承：`directory_order` 的显式字段优先于最近的 `recursive_order`，再继承更高祖先与全局 `order`；显式 `[]` 清空继承。完整 `sequence` 先于 top/bottom 匹配。递归规则仅接受绝对路径或 `~/` 路径，不会误作用于路径前缀相似的其他目录。`O` 保存的顺序只覆盖当前目录，子目录仍按递归规则排列。修改后按 `,` 重载。

实现：`recursive_order` 复用名称模式和 OrderRule，记录 top/bottom/sequence 的显式字段；按目录祖先链逐字段选取最近规则。路径在载入时规范化，比较采用 Path 组件；浏览文件时不增加文件元数据访问。保留 `directory_order` 的单目录用途及注释保留的 O 排序写回。

本次验证：Linux 62 项 Rust、Windows 共享逻辑 60 项 Rust、Linux 20 项 Python／CLI／PTY 检查通过；覆盖三层扩展名排序、名称模式、逐字段继承、最近祖先覆盖、显式 []、相似路径隔离、手动排序不泄漏到后代、配置兼容与既有核心操作。Windows release 与测试程序已交叉编译，未实机验证。已在本机主配置设置 SBA 递归规则；根目录用 sequence=[] 保留 front-end / back-end / docs 置顶，配置软链接不变。

新建目录可用 `A`、`Ctrl+Shift+N` 或 `Ctrl+Alt+N`；`Ctrl+N` 新建文件。tmux 的 CSI-u 控制字符编码会保留 Alt／Shift，避免组合键被误判为 Ctrl+N。如果终端只发送无修饰的 Ctrl+N 字节，filex 无法恢复已丢失的 Shift，可使用 Ctrl+Alt+N 或 A。配置中若显式把 Ctrl+Alt+N 绑定到其他动作，则尊重该绑定。

2026-10-31 tmux 新建目录修饰键修复：真实连接客户端可复现 ESC + Ctrl+N 经 tmux 编成 CSI-u 控制码 14 + Alt，原字符归一化丢失 Alt 后误进入新建文件。现在先还原控制字符隐含的 Control，保留 Alt/Shift，并识别大写字符承载的 Shift；Ctrl+Alt+N 为新建目录默认补充快捷键，显式配置绑定优先，Ctrl+N 保持新建文件。Linux 63 / Windows 共享逻辑 61 项 Rust 测试、23 项 tmux/TTY/导航回归与 2 项真实客户端／直接输入检查通过。已重新构建 Linux / Windows release 及 Windows 测试程序；Windows 未实机验证。tmux 3.8 客户端不接收 Kitty alternate-key 语法，该输入形式只在直接连接 filex 时测试。未改变用户 tmux 配置；未验证物理 Konsole 按键。

文件夹 `q` 面板显示所有层级的普通文件大小总和（逻辑字节数），包括隐藏文件和 filex 隐藏项，不受树展开／聚焦影响；不包含目录记录本身，不跟随软链接或 Windows reparse/junction，硬链接按路径分别计数。右侧统计在后台进行，完成后自动更新，切换目录取消旧统计；`F5`／`Ctrl+R` 重新计算。无法读取的项目显示 `INCOMPLETE` 及跳过数，不能视为完整总量。这不是磁盘分配块用量，稀疏／压缩文件的实际磁盘占用可能不同。

2026-10-31 文件夹大小统计验证：Linux 66 项 Rust、Windows 共享逻辑 64 项 Rust、24 项 Python 元数据/核心检查与 5 项 Linux 文件夹大小/终端回归通过。实际 PTY 覆盖隐藏与视图排除项、多级文件、硬链接按路径计数、软链接循环/FIFO跳过、统计完成自动重绘、修改文件后刷新、切换空目录。单个可取消后台线程以深度优先迭代扫描，不缓存整棵目录；普通 q 面板异步，I 分页及 metadata API 按需同步计算。最新 release 的 8 个内存场景均小于 15 MB，最高 VmHWM 7932 KiB（8.122 MB，十进制）；原始结果与 SHA256 见 Linux memory-validation.json。Windows release / 测试程序已交叉编译，未实机验证。
