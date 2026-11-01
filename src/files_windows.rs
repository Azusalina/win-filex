#[path = "folder_size.rs"]
pub mod folder_size;
use crate::{
    config::{Config, expand_home, product_root},
    model::clean,
    terminal_profile::label,
};
use std::{
    fs::{self, File, FileTimes, OpenOptions},
    io::{self, Read},
    os::windows::{
        ffi::OsStrExt,
        fs::{FileTypeExt, symlink_dir, symlink_file},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::MoveFileExW,
    System::{DataExchange::*, Memory::*, Pipes::PeekNamedPipe},
    UI::Shell::ShellExecuteW,
};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
fn rename_io(source: &Path, dest: &Path) -> io::Result<()> {
    if unsafe { MoveFileExW(wide(source).as_ptr(), wide(dest).as_ptr(), 0) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn rename(source: &Path, dest: &Path) -> Result<(), String> {
    name_valid(
        dest.file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid name")?,
    )?;
    rename_io(source, dest).map_err(err)
}
pub fn replace_staged(source: &Path, dest: &Path) -> Result<(), String> {
    use windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING;
    if unsafe {
        MoveFileExW(
            wide(source).as_ptr(),
            wide(dest).as_ptr(),
            MOVEFILE_REPLACE_EXISTING,
        )
    } == 0
    {
        Err(io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}
pub fn name_valid(name: &str) -> Result<(), String> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|n| {
                ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"].contains(&n)
            });
    if name.is_empty()
        || [".", ".."].contains(&name)
        || name.ends_with([' ', '.'])
        || reserved
        || name.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c))
    {
        Err(
            "Invalid Windows name (reserved name, trailing space/dot, or forbidden character)"
                .into(),
        )
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
    name_valid(
        dest.file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid destination name")?,
    )?;
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
        let target = fs::read_link(source).map_err(err)?;
        if info.file_type().is_symlink_dir() {
            symlink_dir(target, dest).map_err(err)?;
        } else {
            symlink_file(target, dest).map_err(err)?;
        }
    } else if crate::platform::reparse(&info) {
        return Err("Unsupported reparse point: copy cancelled without following it".into());
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
            Err(e) if e.raw_os_error() == Some(17) => {}
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
    // Never fall back to permanent deletion when recycling fails.
    use windows::{
        Win32::{System::Com::*, UI::Shell::*},
        core::PCWSTR,
    };
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let path = crate::platform::shell_path(&std::path::absolute(path).map_err(err)?);
    let name = wide(&path);
    let result = (|| -> windows::core::Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            let _com = Com;
            let operation: IFileOperation =
                CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)?;
            operation.SetOperationFlags(
                FOF_SILENT
                    | FOF_NOERRORUI
                    | FOF_NOCONFIRMATION
                    | FOF_WANTNUKEWARNING
                    | FOFX_EARLYFAILURE
                    | FOFX_RECYCLEONDELETE
                    | FOFX_ADDUNDORECORD,
            )?;
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(name.as_ptr()), None)?;
            operation.DeleteItem(&item, None)?;
            operation.PerformOperations()?;
            if operation.GetAnyOperationsAborted()?.as_bool() {
                return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                    0x80004004u32 as i32,
                )));
            }
            Ok(())
        }
    })();
    result.map_err(err)
}
pub fn open_system(path: &Path) -> Result<(), String> {
    let path = crate::platform::shell_path(&std::path::absolute(path).map_err(err)?);
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            wide(&path).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    } as isize;
    if result <= 32 {
        Err(format!(
            "Windows could not open this path (ShellExecute {result})"
        ))
    } else {
        Ok(())
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
fn date(time: io::Result<std::time::SystemTime>) -> String {
    let Ok(seconds) = time.and_then(|t| {
        t.duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)
    }) else {
        return "Unavailable".into();
    };
    let seconds = seconds.as_secs();
    // Civil-date conversion; UTC is explicit and independent of shell locale.
    let z = (seconds / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
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
        let created = date(info.created());
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
            date(info.modified()),
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
    executable(command).is_some()
}
fn executable(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    let candidates = |base: PathBuf| {
        let mut paths = vec![base.clone()];
        if base.extension().is_none() {
            for ext in ["exe", "com", "cmd", "bat"] {
                paths.push(base.with_extension(ext));
            }
        }
        paths
    };
    if path.is_absolute() || command.contains(['/', '\\']) {
        return candidates(path.to_owned())
            .into_iter()
            .find(|p| p.is_file());
    }
    std::env::var_os("PATH").and_then(|value| {
        std::env::split_paths(&value)
            .flat_map(|p| candidates(p.join(command)))
            .find(|p| p.is_file())
    })
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
    let program = executable(&args[0]).ok_or("Program not installed")?;
    let mut command = Command::new(program);
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
    let absolute = crate::platform::shell_path(&std::path::absolute(path).map_err(err)?);
    let data = wide(&absolute);
    unsafe {
        if OpenClipboard(windows_sys::Win32::System::Console::GetConsoleWindow()) == 0 {
            return Err(err(io::Error::last_os_error()));
        }
        let result = (|| {
            let memory = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2);
            if memory.is_null() {
                return Err(err(io::Error::last_os_error()));
            }
            let pointer = GlobalLock(memory);
            if pointer.is_null() {
                GlobalFree(memory);
                return Err(err(io::Error::last_os_error()));
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), pointer.cast::<u16>(), data.len());
            GlobalUnlock(memory);
            if EmptyClipboard() == 0 || SetClipboardData(13, memory).is_null() {
                GlobalFree(memory);
                return Err(err(io::Error::last_os_error()));
            }
            Ok(())
        })();
        CloseClipboard();
        result
    }
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
    std::path::absolute(directory)
        .map(|p| crate::platform::shell_path(&p))
        .map_err(err)
}

/// Communicate one UTF-8 pathname line to the waiting Windows shell wrapper.
pub fn exit_directory(path: &Path) -> Result<PathBuf, String> {
    let directory = exit_directory_path(path)?;
    if let Some(target) = std::env::var_os("FILEX_CWD_FILE") {
        let output = format!("{}\n", directory.to_string_lossy()).into_bytes();
        fs::write(target, output).map_err(err)?;
    }
    Ok(directory)
}

fn nonblocking(_: &impl AsRawHandle) -> Result<(), String> {
    Ok(())
}
fn drain_pipe(stream: &mut (impl Read + AsRawHandle), output: &mut String) {
    let mut bytes = [0u8; 2048];
    while output.len() < 16384 {
        let mut available = 0;
        if unsafe {
            PeekNamedPipe(
                stream.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        } == 0
            || available == 0
        {
            break;
        }
        let length = bytes.len().min(available as usize);
        match stream.read(&mut bytes[..length]) {
            Ok(0) | Err(_) => break,
            Ok(n) => output.push_str(&String::from_utf8_lossy(&bytes[..n])),
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
