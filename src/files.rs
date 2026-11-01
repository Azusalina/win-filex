#[path = "folder_size.rs"]
pub mod folder_size;
use crate::{
    config::{Config, expand_home, product_root},
    model::clean,
    terminal_profile::label,
};
use std::{
    ffi::CString,
    fs::{self, File, FileTimes, OpenOptions},
    io::{self, Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, symlink},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn cname(path: &Path) -> Result<CString, String> {
    CString::new(path.as_os_str().as_bytes()).map_err(err)
}
pub fn rename(source: &Path, dest: &Path) -> Result<(), String> {
    let s = cname(source)?;
    let d = cname(dest)?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            s.as_ptr(),
            libc::AT_FDCWD,
            d.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        Err(io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}
fn rename_io(source: &Path, dest: &Path) -> io::Result<()> {
    let s = cname(source).map_err(io::Error::other)?;
    let d = cname(dest).map_err(io::Error::other)?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            s.as_ptr(),
            libc::AT_FDCWD,
            d.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn replace_staged(source: &Path, dest: &Path) -> Result<(), String> {
    fs::rename(source, dest).map_err(err)
}
pub fn name_valid(name: &str) -> Result<(), String> {
    if name.is_empty() || [".", ".."].contains(&name) || name.contains('/') || name.contains('\0') {
        Err(label("请输入单个有效名称", "Enter one valid name").into())
    } else {
        Ok(())
    }
}
pub fn create(directory: &Path, name: &str, folder: bool) -> Result<PathBuf, String> {
    name_valid(name)?;
    let path = directory.join(name);
    if folder {
        fs::create_dir(&path).map_err(err)?
    } else {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(err)?;
    }
    Ok(path)
}
pub fn destination(source: &Path, text: &str) -> PathBuf {
    let path = expand_home(text);
    if path.is_absolute() {
        path
    } else {
        source.parent().unwrap_or(Path::new("/")).join(path)
    }
}
pub fn validate(source: &Path, dest: &Path) -> Result<(), String> {
    if fs::symlink_metadata(dest).is_ok() {
        return Err(ui_format!(
            "目标已存在: {}",
            "Destination already exists: {}",
            dest.display()
        ));
    }
    let parent = dest.parent().ok_or("目标父目录无效")?;
    let parent = fs::canonicalize(parent).map_err(err)?;
    if !parent.is_dir() {
        return Err("目标父目录不是目录".into());
    }
    let info = fs::symlink_metadata(source).map_err(err)?;
    if info.is_dir()
        && parent
            .join(dest.file_name().ok_or("无效目标名称")?)
            .starts_with(fs::canonicalize(source).map_err(err)?)
    {
        return Err("不能将目录复制 / 移动到自身内部".into());
    }
    Ok(())
}
fn copy_one(source: &Path, dest: &Path, depth: usize) -> Result<(), String> {
    if depth > 128 {
        return Err("复制目录超过 128 层；源保留，目标可能包含部分副本".into());
    }
    let info = fs::symlink_metadata(source).map_err(err)?;
    if info.file_type().is_symlink() {
        symlink(fs::read_link(source).map_err(err)?, dest).map_err(err)?
    } else if info.is_dir() {
        fs::create_dir(dest).map_err(err)?;
        for item in fs::read_dir(source).map_err(err)? {
            let item = item.map_err(err)?;
            copy_one(&item.path(), &dest.join(item.file_name()), depth + 1)?
        }
        fs::set_permissions(dest, info.permissions()).map_err(err)?;
    } else if info.is_file() {
        let mut input = File::open(source).map_err(err)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(err)?;
        io::copy(&mut input, &mut output).map_err(err)?;
        output.set_permissions(info.permissions()).map_err(err)?;
        let mut times = FileTimes::new();
        if let Ok(time) = info.modified() {
            times = times.set_modified(time)
        }
        if let Ok(time) = info.accessed() {
            times = times.set_accessed(time)
        }
        output.set_times(times).map_err(err)?;
    } else {
        return Err("仅支持普通文件、目录和符号链接".into());
    }
    Ok(())
}
pub fn transfer(source: &Path, dest: &Path, moving: bool) -> Result<(), String> {
    validate(source, dest)?;
    if moving {
        match rename_io(source, dest) {
            Ok(()) => return Ok(()),
            Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {}
            Err(e) => return Err(err(e)),
        }
    }
    copy_one(source, dest, 0)
        .map_err(|e| format!("{e}；源保留，请检查可能存在的部分目标: {}", dest.display()))?;
    if moving {
        let info = fs::symlink_metadata(source).map_err(err)?;
        if info.is_dir() {
            fs::remove_dir_all(source).map_err(err)?
        } else {
            fs::remove_file(source).map_err(err)?
        }
    }
    Ok(())
}
pub fn trash(path: &Path) -> Result<(), String> {
    let output = Command::new("gio")
        .args(["trash", "--"])
        .arg(path)
        .output()
        .map_err(err)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "回收站操作失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
pub fn preview(path: &Path, limit: usize) -> String {
    let read = || -> Result<String, String> {
        let info = fs::symlink_metadata(path).map_err(err)?;
        if info.file_type().is_symlink() {
            return Ok(ui_format!(
                "符号链接 → {}",
                "Symbolic link -> {}",
                clean(&fs::read_link(path).map_err(err)?.to_string_lossy())
            ));
        }
        if info.is_dir() {
            return Ok(ui_format!(
                "目录\n{}\n展开以查看内容",
                "Directory\n{}\nExpand to see contents",
                clean(&path.to_string_lossy())
            ));
        }
        if !info.is_file() {
            return Ok(label(
                "特殊文件（设备、管道或套接字），不读取内容",
                "Special file (device, pipe or socket); contents not read",
            )
            .into());
        }
        let mut data = Vec::new();
        File::open(path)
            .map_err(err)?
            .take((limit + 1) as u64)
            .read_to_end(&mut data)
            .map_err(err)?;
        let truncated = data.len() > limit;
        data.truncate(limit);
        let text = match std::str::from_utf8(&data) {
            Ok(v) => v,
            Err(e) if truncated && e.error_len().is_none() => {
                std::str::from_utf8(&data[..e.valid_up_to()]).map_err(err)?
            }
            _ => {
                return Ok(ui_format!(
                    "{} bytes\n非 UTF-8 文本 / 二进制文件",
                    "{} bytes\nNon-UTF-8 text / binary file",
                    info.len()
                ));
            }
        };
        if data.contains(&0) {
            return Ok(ui_format!(
                "{} bytes\n二进制文件，用默认程序打开",
                "{} bytes\nBinary file; choose an appropriate opener",
                info.len()
            ));
        }
        Ok(format!(
            "{} bytes\n\n{}{}",
            info.len(),
            text.lines()
                .map(|line| clean(&line.replace('\t', "    ")))
                .collect::<Vec<_>>()
                .join("\n"),
            if truncated {
                label("\n…预览已截断", "\n...preview truncated")
            } else {
                ""
            }
        ))
    };
    read().unwrap_or_else(|e| ui_format!("无法预览: {e}", "Cannot preview: {e}"))
}
fn date(seconds: i64) -> String {
    let time = seconds as libc::time_t;
    let mut tm = unsafe { std::mem::zeroed::<libc::tm>() };
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return "不可用".into();
    }
    let mut buffer = [0u8; 64];
    let format = c"%Y-%m-%d %H:%M:%S %Z";
    let length = unsafe {
        libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            format.as_ptr(),
            &tm,
        )
    };
    String::from_utf8_lossy(&buffer[..length]).into_owned()
}
pub fn metadata(path: &Path, root: &Path, conf: &Config) -> String {
    metadata_with_size(path, root, conf, None)
}
fn metadata_with_size(
    path: &Path,
    root: &Path,
    conf: &Config,
    folder_total: Option<&str>,
) -> String {
    let read = || -> Result<String, String> {
        let info = fs::symlink_metadata(path).map_err(err)?;
        let name = cname(path)?;
        let mut extended = unsafe { std::mem::zeroed::<libc::statx>() };
        let born = unsafe {
            libc::statx(
                libc::AT_FDCWD,
                name.as_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
                libc::STATX_BTIME,
                &mut extended,
            )
        };
        let created = if born == 0 && extended.stx_mask & libc::STATX_BTIME != 0 {
            date(extended.stx_btime.tv_sec)
        } else {
            label(
                "不可用（文件系统未提供）",
                "Unavailable (not provided by filesystem)",
            )
            .into()
        };
        let kind = if info.is_dir() {
            label("目录", "Directory")
        } else if info.file_type().is_symlink() {
            label("符号链接", "Symbolic link")
        } else {
            label("文件", "File")
        };
        let note = if info.is_dir() && folder_size::directory(path) {
            label(
                "递归文件总大小；含隐藏项，不跟随链接；硬链接按路径计数",
                "All nested files; includes hidden; no link traversal; hard links per path",
            )
        } else if info.file_type().is_symlink() || info.is_dir() {
            label("链接自身大小；不跟随目标", "Link metadata size; target not followed")
        } else {
            ""
        };
        Ok(ui_format!(
            "{}\n{kind}\n\nCreated / 创建时间\n{created}\n\nModified / 修改时间\n{}\n\nSize / 大小\n{}\n{note}\n\n{}\nPath / 路径\n{}",
            "{}\n{kind}\n\nCreated\n{created}\n\nModified\n{}\n\nSize\n{}\n{note}\n\n{}\nPath\n{}",
            clean(
                &path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
            ),
            date(info.mtime()),
            if info.is_dir() && folder_size::directory(path) {
                folder_total
                    .map(str::to_owned)
                    .unwrap_or_else(|| folder_size::synchronous(path))
            } else {
                format!("{} bytes", info.len())
            },
            crate::links::details(path, root, &info, conf),
            clean(&path.to_string_lossy())
        ))
    };
    read().unwrap_or_else(|e| ui_format!("无法读取信息: {e}", "Cannot read information: {e}"))
}
pub fn available(command: &str) -> bool {
    let executable =
        |path: &Path| fs::metadata(path).is_ok_and(|m| m.is_file() && m.mode() & 0o111 != 0);
    if command.contains('/') {
        return executable(Path::new(command));
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|p| executable(&p.join(command))))
}
/// A script extension alone never selects execution as a headless fallback.
pub fn cli_method<'a>(conf: &'a Config, requested: &'a str) -> Option<&'a str> {
    if conf.openers.get(requested).is_some_and(|o| o.terminal) {
        return Some(requested);
    }
    if conf.cli_opener != "auto" {
        return Some(&conf.cli_opener);
    }
    ["nano", "nvim", "vim"].into_iter().find(|id| {
        conf.openers
            .get(*id)
            .is_some_and(|o| o.terminal && available(&o.command[0]))
    })
}
pub fn open_command(path: &Path, conf: &Config, method: &str) -> Result<(Command, bool), String> {
    let opener = conf.openers.get(method).ok_or("打开程序未定义")?;
    if !opener.supports(path) {
        return Err("This opener does not support the selected file extension".into());
    }
    let root = product_root();
    let toolkit = root.parent().unwrap_or(&root).join("toolkit");
    let mut args = opener
        .command
        .iter()
        .map(|s| {
            s.replace("{filex}", &root.to_string_lossy())
                .replace("{toolkit}", &toolkit.to_string_lossy())
                .replace("{path}", &path.to_string_lossy())
        })
        .collect::<Vec<_>>();
    // Ubuntu/Fedora commonly install python3 without a python compatibility link.
    if method == "python" && args[0] == "python" && !available("python") && available("python3") {
        args[0] = "python3".into();
    }
    if !available(&args[0]) {
        return Err(format!("程序未安装: {}", args[0]));
    }
    let mut command = Command::new(&args[0]);
    command.args(&args[1..]).env_remove("FILEX_CWD_FILE");
    if opener.terminal {
        command.current_dir(if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(Path::new("."))
        });
    }
    if opener.copy_path && crate::terminal_profile::get().desktop {
        copy_path(path)?;
    }
    if !opener.terminal {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }
    Ok((command, opener.terminal))
}

/// Send the selected pathname as text without resolving a symlink or adding a newline.
pub fn copy_path(path: &Path) -> Result<(), String> {
    let absolute = std::path::absolute(path).map_err(err)?;
    let mut child = Command::new("wl-copy")
        .args(["--type", "text/plain;charset=utf-8"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("复制路径需要 wl-clipboard / Wayland: {e}"))?;
    let written = child
        .stdin
        .take()
        .unwrap()
        .write_all(absolute.as_os_str().as_bytes());
    // wl-copy daemonizes; its clipboard owner inherits stderr. Waiting for EOF
    // would block until somebody replaces the clipboard, even after success.
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(err)? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("复制路径超时，请检查 Wayland 剪贴板服务".into());
        }
        thread::sleep(Duration::from_millis(10));
    };
    if !status.success() {
        let mut errors = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            nonblocking(&stderr)?;
            drain_pipe(&mut stderr, &mut errors);
        }
        return Err(format!("复制路径失败: {}", errors.trim()));
    }
    written.map_err(err)
}

/// Resolve relative / chained symlinks only when their target is selected.
pub fn link_target(path: &Path) -> Result<PathBuf, String> {
    fs::read_link(path).map_err(|e| format!("读取链接目标失败: {e}"))?;
    fs::canonicalize(path).map_err(|e| format!("链接目标不可访问: {e}"))
}

pub fn exit_directory_path(path: &Path) -> Result<PathBuf, String> {
    fs::symlink_metadata(path).map_err(err)?;
    let directory = if path.is_dir() {
        path
    } else {
        path.parent().ok_or("无法找到所在目录")?
    };
    if !directory.is_dir() {
        return Err("目标目录已移除或不可访问".into());
    }
    std::path::absolute(directory).map_err(err)
}

/// Communicate a NUL-terminated pathname to the waiting shell function.
pub fn exit_directory(path: &Path) -> Result<PathBuf, String> {
    let directory = exit_directory_path(path)?;
    if let Some(target) = std::env::var_os("FILEX_CWD_FILE") {
        let mut output = directory.as_os_str().as_bytes().to_vec();
        output.push(0);
        fs::write(target, output).map_err(err)?;
    }
    Ok(directory)
}

fn nonblocking(stream: &impl AsRawFd) -> Result<(), String> {
    let fd = stream.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(err(io::Error::last_os_error()));
    }
    Ok(())
}
fn drain_pipe(stream: &mut impl Read, output: &mut String) {
    let mut bytes = [0; 2048];
    while output.len() < 16384 {
        match stream.read(&mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(count) => output.push_str(&String::from_utf8_lossy(&bytes[..count])),
        }
    }
}
/// The bundled adapter prints its URL only after server and browser startup.
pub fn start_markdown(command: &mut Command) -> Result<String, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(err)?;
    let mut stdout = child.stdout.take().ok_or("Markdown 没有启动输出")?;
    let mut stderr = child.stderr.take().ok_or("Markdown 没有错误输出")?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut output = String::new();
    let mut errors = String::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        drain_pipe(&mut stdout, &mut output);
        drain_pipe(&mut stderr, &mut errors);
        if let Some(url) = output
            .lines()
            .find(|line| line.starts_with("http://127.0.0.1:"))
        {
            return Ok(url.to_owned());
        }
        if let Some(status) = child.try_wait().map_err(err)? {
            drain_pipe(&mut stderr, &mut errors);
            return Err(format!("Markdown 启动失败 ({status}): {}", errors.trim()));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Markdown 启动超时: {}", errors.trim()));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Dashboard text with a bounded text head; I opens the same content in a pager.
pub fn dashboard(path: &Path, root: &Path, conf: &Config) -> String {
    dashboard_with_size(path, root, conf, None)
}
pub fn dashboard_with_size(
    path: &Path,
    root: &Path,
    conf: &Config,
    folder_total: Option<&str>,
) -> String {
    let metadata = if folder_total.is_none() {
        metadata(path, root, conf)
    } else {
        metadata_with_size(path, root, conf, folder_total)
    };
    let compact = metadata
        .lines()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if conf.preview_lines == 0 || !fs::symlink_metadata(path).is_ok_and(|info| info.is_file()) {
        return compact;
    }
    let preview = preview(path, conf.preview_bytes);
    let body = preview
        .split_once("\n\n")
        .map_or(preview.as_str(), |(_, body)| body);
    let lines = body
        .lines()
        .take(conf.preview_lines)
        .collect::<Vec<_>>()
        .join("\n");
    ui_format!(
        "Preview / 前 {} 行\n{}\n\n{compact}",
        "Preview / first {} lines\n{}\n\n{compact}",
        conf.preview_lines,
        if lines.is_empty() {
            label("（空文件）", "(empty file)")
        } else {
            &lines
        }
    )
}
