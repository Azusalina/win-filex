use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};
use toml::Value;

#[derive(Clone, Debug)]
pub struct Opener {
    pub label: String,
    pub command: Vec<String>,
    pub terminal: bool,
    pub copy_path: bool,
    pub pause_on_exit: bool,
    pub extensions: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct OrderRule {
    pub specified: u8, // top=1, bottom=2, sequence=4; scoped fields inherit individually.
    pub top: Vec<String>,
    pub bottom: Vec<String>,
    pub sequence: Vec<String>,
}
fn order_names(value: &Value) -> Result<Vec<String>, String> {
    value
        .as_array()
        .ok_or("Order must be a list of names")?
        .iter()
        .map(|v| {
            let name = string(v, "order name")?;
            if name.is_empty()
                || name.contains(['/', '\\', '\0'])
                || [".", ".."].contains(&name.as_str())
            {
                return Err("Order entries must be single file/folder names".into());
            }
            Ok(name)
        })
        .collect()
}
#[derive(Clone, Debug)]
pub struct Config {
    pub hidden: bool,
    pub dirs_first: bool,
    pub sort: String,
    pub max_entries: usize,
    pub max_expand: usize,
    pub max_loaded: usize,
    pub gap: usize,
    pub label_width: usize,
    pub row_spacing: usize,
    pub highlight_path: bool,
    pub grouped: bool,
    pub excluded_paths: Vec<PathBuf>,
    pub tmux_right: bool,
    pub auto_english: bool,
    pub konsole_shortcuts: bool,
    pub tmux_extended_keys: bool,
    pub terminal_mode: String,
    pub color_mode: String,
    pub charset: String,
    pub cli_opener: String,
    pub link_scan_limit: usize,
    pub link_results: usize,
    pub preview_bytes: usize,
    pub preview_lines: usize,
    pub dashboard_width: usize,
    pub search_results: usize,
    pub search_limit: usize,
    pub folder_color: String,
    pub folder_colors: BTreeMap<String, String>,
    pub file_color: String,
    pub line_color: String,
    pub link_color: String,
    pub path_color: String,
    pub mark_color: String,
    pub order: OrderRule,
    pub directory_order: BTreeMap<String, OrderRule>,
    pub recursive_order: BTreeMap<String, OrderRule>,
    pub git_enabled: bool,
    pub git_max_entries: usize,
    pub keys: BTreeMap<String, String>,
    pub openers: BTreeMap<String, Opener>,
    pub defaults: BTreeMap<String, String>,
    pub extension_colors: BTreeMap<String, String>,
}
pub fn config_path() -> PathBuf {
    #[cfg(windows)]
    return env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("AppData/Roaming"))
        .join("win-filex/filex.conf");
    #[cfg(not(windows))]
    {
        let directory = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".config"))
            .join("fileX");
        let path = directory.join("filex.conf");
        let legacy = directory.join("fileX.conf");
        if !path.exists() && legacy.exists() {
            legacy
        } else {
            path
        }
    }
}
pub fn product_root() -> PathBuf {
    env::var_os("FILEX_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}
fn home() -> PathBuf {
    PathBuf::from(
        env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap_or_default(),
    )
}
pub fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        home()
    } else if let Some(tail) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        home().join(tail)
    } else {
        PathBuf::from(value)
    }
}
pub fn key_code(value: &str) -> String {
    if value.eq_ignore_ascii_case("Tab") {
        return "\t".into();
    }
    let lowered = value.to_ascii_lowercase();
    if lowered == "ctrl+f5" {
        return lowered;
    }
    for prefix in ["ctrl+alt+", "ctrl+shift+", "ctrl+alt+shift+"] {
        if lowered.len() == prefix.len() + 1
            && lowered.starts_with(prefix)
            && lowered.as_bytes()[prefix.len()].is_ascii_alphabetic()
        {
            return lowered;
        }
    }
    if value.eq_ignore_ascii_case("Left") {
        return "left".into();
    }
    if value.len() == 6
        && value.as_bytes()[..5].eq_ignore_ascii_case(b"ctrl+")
        && value.as_bytes()[5].is_ascii_alphabetic()
    {
        return char::from(value.as_bytes()[5].to_ascii_lowercase() - b'a' + 1).to_string();
    }
    value.to_owned()
}
impl Opener {
    pub fn supports(&self, path: &Path) -> bool {
        self.extensions.is_empty()
            || !path.is_dir()
                && path.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy().to_lowercase();
                    self.extensions
                        .iter()
                        .any(|extension| name.ends_with(extension))
                })
    }
}
impl Default for Config {
    fn default() -> Self {
        let theme: Value = toml::from_str(include_str!("../themes/terminal-16.conf"))
            .expect("valid embedded terminal theme");
        let theme_map = |section: &str| -> BTreeMap<String, String> {
            theme[section]
                .as_table()
                .expect("theme table")
                .iter()
                .map(|(key, value)| (key.clone(), value.as_str().expect("theme color").to_owned()))
                .collect()
        };
        let theme_color = |key: &str| {
            theme["colors"][key]
                .as_str()
                .expect("theme color")
                .to_owned()
        };
        let keys = [
            ("quit", "Q"),
            ("exit_here", "x"),
            ("dashboard", "q"),
            ("dashboard_narrow", "["),
            ("dashboard_widen", "]"),
            ("link_info", "I"),
            ("expand", "l"),
            ("collapse", "h"),
            ("up", "k"),
            ("down", "j"),
            ("expand_all", "E"),
            ("collapse_all", "C"),
            ("open", "o"),
            ("open_with", "w"),
            ("preview", "p"),
            ("hidden", "."),
            ("refresh", "Ctrl+r"),
            ("search", "Ctrl+f"),
            ("next_match", "n"),
            ("previous_match", "N"),
            ("goto", "g"),
            ("parent", "Left"),
            ("focus_root", "f"),
            ("temporary_focus", "F"),
            ("new_file", "a"),
            ("new_folder", "A"),
            ("rename", "r"),
            ("copy", "c"),
            ("copy_path", "Ctrl+Alt+c"),
            ("move", "m"),
            ("trash", "D"),
            ("run_background", "!"),
            ("run_foreground", "@"),
            ("git_menu", "G"),
            ("help", "?"),
            ("config", ","),
            ("toggle_grouped", "v"),
            ("reorder", "O"),
            ("hide_item", "z"),
            ("hidden_items", "Z"),
        ]
        .into_iter()
        .map(|(a, b)| (a.into(), b.into()))
        .collect();
        let openers: BTreeMap<String, Opener> = [
            ("system", "系统默认", vec!["xdg-open", "{path}"], false),
            (
                "vscodium",
                "VSCodium",
                vec!["codium", "--", "{path}"],
                false,
            ),
            ("vim", "Vim", vec!["vim", "--", "{path}"], true),
            ("nvim", "Neovim", vec!["nvim", "--", "{path}"], true),
            ("nano", "Nano", vec!["nano", "--", "{path}"], true),
            ("firefox", "Firefox", vec!["firefox", "{path}"], false),
            ("python", "Python 执行", vec!["python", "{path}"], true),
            ("rscript", "Rscript 执行", vec!["Rscript", "{path}"], true),
            ("bash", "Bash 执行", vec!["bash", "--", "{path}"], true),
            (
                "unzip",
                "Bash unzip / 解压 ZIP",
                vec!["bash", "--", "{filex}/scripts/unzip.sh", "{path}"],
                true,
            ),
            (
                "markdown",
                "Toolkit Markdown / Firefox",
                vec![
                    "node",
                    "{filex}/scripts/open_markdown.mjs",
                    "--toolkit",
                    "{toolkit}",
                    "--browser",
                    "firefox",
                    "--",
                    "{path}",
                ],
                false,
            ),
        ]
        .into_iter()
        .map(|(id, label, command, terminal)| {
            (
                id.into(),
                Opener {
                    label: label.into(),
                    command: command.into_iter().map(str::to_string).collect(),
                    terminal,
                    copy_path: id == "markdown",
                    extensions: match id {
                        "python" => vec![".py", ".pyw"],
                        "rscript" => vec![".r"],
                        "bash" => vec![".sh", ".bash"],
                        "unzip" => vec![".zip"],
                        "markdown" => vec![".md", ".markdown"],
                        _ => vec![],
                    }
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                    pause_on_exit: ["python", "rscript", "bash", "unzip"].contains(&id),
                },
            )
        })
        .collect();
        #[cfg(windows)]
        let openers = {
            let mut openers = openers;
            openers.get_mut("system").unwrap().command = vec![
                std::env::current_exe()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                "--open-system".into(),
                "{path}".into(),
            ];
            openers.insert(
                "notepad".into(),
                Opener {
                    label: "Notepad".into(),
                    command: vec!["notepad.exe".into(), "{path}".into()],
                    terminal: false,
                    copy_path: false,
                    pause_on_exit: false,
                    extensions: vec![],
                },
            );
            openers.insert(
                "powershell".into(),
                Opener {
                    label: "PowerShell script".into(),
                    command: vec![
                        "powershell.exe".into(),
                        "-NoProfile".into(),
                        "-File".into(),
                        "{path}".into(),
                    ],
                    terminal: true,
                    copy_path: false,
                    pause_on_exit: true,
                    extensions: vec![".ps1".into()],
                },
            );
            openers
        };
        Self {
            hidden: false,
            dirs_first: true,
            sort: "name".into(),
            max_entries: 1000,
            max_expand: 5000,
            max_loaded: 6000,
            gap: 6,
            label_width: 32,
            row_spacing: 1,
            highlight_path: true,
            grouped: false,
            excluded_paths: vec![],
            tmux_right: !cfg!(windows),
            auto_english: !cfg!(windows),
            konsole_shortcuts: !cfg!(windows),
            tmux_extended_keys: !cfg!(windows),
            terminal_mode: "auto".into(),
            color_mode: if cfg!(windows) {
                "truecolor"
            } else {
                "consistent"
            }
            .into(),
            charset: "auto".into(),
            cli_opener: "auto".into(),
            link_scan_limit: 10000,
            link_results: 32,
            preview_bytes: 65536,
            preview_lines: 5,
            dashboard_width: 36,
            search_results: 500,
            search_limit: 100000,
            folder_color: theme_color("folder_color"),
            folder_colors: theme_map("folder_colors"),
            file_color: theme_color("file_color"),
            line_color: theme_color("line_color"),
            link_color: theme_color("link_color"),
            path_color: theme_color("path_color"),
            mark_color: "#FF55FF".into(),
            order: Default::default(),
            directory_order: Default::default(),
            recursive_order: Default::default(),
            git_enabled: true,
            git_max_entries: 4000,
            keys,
            openers,
            defaults: [
                (".py", if cfg!(windows) { "notepad" } else { "vscodium" }),
                (".html", if cfg!(windows) { "system" } else { "firefox" }),
                (".htm", if cfg!(windows) { "system" } else { "firefox" }),
                (".md", if cfg!(windows) { "system" } else { "markdown" }),
                (
                    ".markdown",
                    if cfg!(windows) { "system" } else { "markdown" },
                ),
                (".zip", "unzip"),
                ("*", "system"),
            ]
            .into_iter()
            .map(|(a, b)| (a.into(), b.into()))
            .collect(),
            extension_colors: theme_map("extension_colors"),
        }
    }
}
fn color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}
fn string(value: &Value, key: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{key}: 应为字符串"))
}
fn choice(value: &Value, key: &str, choices: &[&str]) -> Result<String, String> {
    let value = string(value, key)?;
    if choices.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(format!("{key}: {}", choices.join(" / ")))
    }
}
fn number(value: &Value, key: &str, low: usize, high: usize) -> Result<usize, String> {
    value
        .as_integer()
        .filter(|&n| n >= low as i64 && n <= high as i64)
        .map(|n| n as usize)
        .ok_or_else(|| format!("{key}: 应为 {low}..{high} 的整数"))
}
impl Config {
    pub fn order_rank(&self, directory: &Path, name: &str, is_dir: bool) -> (i8, usize) {
        let full = directory.to_string_lossy();
        let base = directory.file_name().unwrap_or_default().to_string_lossy();
        let direct = self
            .directory_order
            .get(full.as_ref())
            .or_else(|| self.directory_order.get(base.as_ref()));
        // Select each field independently: exact directory, nearest ancestor, global.
        let rule = |field| {
            direct.filter(|r| r.specified & field != 0).or_else(|| {
                directory.ancestors().find_map(|parent| {
                    self.recursive_order
                        .get(parent.to_string_lossy().as_ref())
                        .filter(|r| r.specified & field != 0)
                })
            })
        };
        let top = rule(1).map_or(&self.order.top, |r| &r.top);
        let bottom = rule(2).map_or(&self.order.bottom, |r| &r.bottom);
        let sequence = rule(4).map_or(&self.order.sequence, |r| &r.sequence);
        if let Some(index) = sequence
            .iter()
            .position(|p| crate::ordering::matches(p, name, is_dir))
        {
            return (-2, index);
        }
        for (group, list) in [(-1, top), (1, bottom)] {
            if let Some(index) = list.iter().position(|p| crate::ordering::exact(p, name)) {
                return (group, index);
            }
        }
        for (group, list) in [(-1, top), (1, bottom)] {
            if let Some(index) = list
                .iter()
                .position(|p| crate::ordering::matches(p, name, is_dir))
            {
                return (group, index);
            }
        }
        (0, 0)
    }

    pub fn dashboard_columns(&self, terminal_width: usize) -> usize {
        if terminal_width < 70 {
            0
        } else {
            self.dashboard_width.min(terminal_width - 38)
        }
    }
    pub fn folder_color_for(&self, path: &Path) -> &str {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| self.folder_colors.get(name))
            .unwrap_or(&self.folder_color)
    }
    pub fn arrow_action(&self, key: &str) -> Option<&'static str> {
        match key {
            "up" => Some("up"),
            "down" => Some("down"),
            "left" => Some("parent"),
            "right" => Some("expand"),
            _ => None,
        }
    }
    pub fn arrow_hint(&self) -> &'static str {
        "↑↓ 同层循环 ← 父级 → 展开"
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let data: Value = toml::from_str(&text).map_err(|e| e.to_string())?;
        let table = data.as_table().ok_or("配置应为 TOML 表")?;
        let mut conf = Self::default();
        if let Some(order) = table.get("order").and_then(Value::as_table) {
            if let Some(sequence) = order.get("sequence") {
                conf.order.sequence = order_names(sequence)?;
            }
            if let Some(top) = order.get("top") {
                conf.order.top = order_names(top)?;
            }
            if let Some(bottom) = order.get("bottom") {
                conf.order.bottom = order_names(bottom)?;
            }
        }
        for (section, values) in table {
            if section == "gui" {
                continue;
            } // Ignore retired configuration.
            if ![
                "browser",
                "layout",
                "preview",
                "dashboard",
                "search",
                "colors",
                "keys",
                "openers",
                "open_defaults",
                "extension_colors",
                "folder_colors",
                "order",
                "directory_order",
                "recursive_order",
                "git",
                "tmux",
                "input_method",
                "links",
                "terminal",
            ]
            .contains(&section.as_str())
            {
                return Err(format!("未知配置区段: {section}"));
            }
            let values = values
                .as_table()
                .ok_or_else(|| format!("{section}: 应为表"))?;
            for (key, value) in values {
                match (section.as_str(), key.as_str()) {
                    ("order", "sequence") => conf.order.sequence = order_names(value)?,
                    ("order", "top") => conf.order.top = order_names(value)?,
                    ("order", "bottom") => conf.order.bottom = order_names(value)?,
                    ("directory_order" | "recursive_order", _) => {
                        let mut rule = OrderRule::default();
                        for (field, names) in
                            value.as_table().ok_or("Ordering entry must be a table")?
                        {
                            match field.as_str() {
                                "sequence" => {
                                    rule.sequence = order_names(names)?;
                                    rule.specified |= 4;
                                }
                                "top" => {
                                    rule.top = order_names(names)?;
                                    rule.specified |= 1;
                                }
                                "bottom" => {
                                    rule.bottom = order_names(names)?;
                                    rule.specified |= 2;
                                }
                                _ => return Err(format!("Unknown ordering field: {field}")),
                            }
                        }
                        let path = expand_home(key);
                        let key = if path.is_absolute() {
                            fs::canonicalize(&path)
                                .unwrap_or(path)
                                .to_string_lossy()
                                .into_owned()
                        } else {
                            if section == "recursive_order" {
                                return Err(
                                    "recursive_order requires an absolute or ~/ path".into()
                                );
                            }
                            if key.contains(['/', '\\']) {
                                return Err(
                                    "Directory rule must be an absolute path or directory name"
                                        .into(),
                                );
                            }
                            key.clone()
                        };
                        if section == "recursive_order" {
                            conf.recursive_order.insert(key, rule);
                        } else {
                            conf.directory_order.insert(key, rule);
                        }
                    }
                    ("git", "enabled") => {
                        conf.git_enabled = value.as_bool().ok_or("git.enabled must be boolean")?
                    }
                    ("git", "max_entries") => {
                        conf.git_max_entries = number(value, key, 100, 20000)?
                    }
                    ("browser", "excluded_paths") => {
                        conf.excluded_paths = value
                            .as_array()
                            .ok_or("excluded_paths must be an array")?
                            .iter()
                            .map(|v| {
                                let p = expand_home(&string(v, "excluded path")?);
                                if !p.is_absolute() {
                                    return Err("Excluded paths must be absolute".into());
                                }
                                Ok(p)
                            })
                            .collect::<Result<_, String>>()?;
                    }
                    ("browser", "show_hidden") => {
                        conf.hidden = value.as_bool().ok_or("show_hidden 应为布尔值")?
                    }
                    ("browser", "directories_first") => {
                        conf.dirs_first = value.as_bool().ok_or("directories_first 应为布尔值")?
                    }
                    ("browser", "sort") => {
                        conf.sort = string(value, key)?;
                        if !["name", "size", "modified"].contains(&conf.sort.as_str()) {
                            return Err("sort 应为 name / size / modified".into());
                        }
                    }
                    ("browser", "max_entries") => conf.max_entries = number(value, key, 1, 100000)?,
                    ("browser", "max_expand_nodes") => {
                        conf.max_expand = number(value, key, 1, 100000)?
                    }
                    ("browser", "max_loaded_nodes") => {
                        conf.max_loaded = number(value, key, 100, 100000)?
                    }
                    ("layout", "column_gap") => conf.gap = number(value, key, 3, 20)?,
                    ("layout", "max_label_width") => {
                        conf.label_width = number(value, key, 12, 120)?
                    }
                    ("layout", "row_spacing") => conf.row_spacing = number(value, key, 1, 4)?,
                    ("layout", "highlight_path") => {
                        conf.highlight_path = value.as_bool().ok_or("highlight_path 应为布尔值")?
                    }
                    ("layout", "grouped") => {
                        conf.grouped = value.as_bool().ok_or("grouped must be boolean")?
                    }
                    ("layout", "dual_pane") | ("keys", "toggle_dual") | ("keys", "switch_pane") => {
                    }
                    // Ignore retired options so existing user configurations still load.
                    ("layout", "orientation") | ("keys", "toggle_tree") => {}
                    ("links", "scan_limit") => {
                        conf.link_scan_limit = number(value, key, 1, 1000000)?
                    }
                    ("links", "max_results") => conf.link_results = number(value, key, 1, 1000)?,
                    ("terminal", "tmux_extended_keys") => {
                        conf.tmux_extended_keys =
                            value.as_bool().ok_or("tmux_extended_keys 应为布尔值")?
                    }
                    ("terminal", "mode") => {
                        conf.terminal_mode = choice(value, key, &["auto", "tty"])?
                    }
                    ("terminal", "color_mode") => {
                        conf.color_mode = choice(
                            value,
                            key,
                            &["consistent", "auto", "16", "256", "truecolor", "none"],
                        )?
                    }
                    ("terminal", "charset") => {
                        conf.charset = choice(value, key, &["auto", "ascii", "unicode"])?
                    }
                    ("terminal", "cli_opener") => conf.cli_opener = string(value, key)?,
                    ("terminal", "konsole_shortcuts") => {
                        conf.konsole_shortcuts =
                            value.as_bool().ok_or("konsole_shortcuts 应为布尔值")?
                    }
                    ("input_method", "auto_english") => {
                        conf.auto_english = value.as_bool().ok_or("auto_english 应为布尔值")?;
                    }
                    ("tmux", "open_in_right_pane") => {
                        conf.tmux_right = value.as_bool().ok_or("open_in_right_pane 应为布尔值")?;
                    }
                    ("preview", "preview_lines") => {
                        conf.preview_lines = number(value, key, 0, 1000)?
                    }
                    ("preview", "preview_bytes") => {
                        conf.preview_bytes = number(value, key, 1024, 1048576)?
                    }
                    ("dashboard", "dashboard_width") => {
                        conf.dashboard_width = number(value, key, 20, 240)?
                    }
                    ("search", "search_max_results") => {
                        conf.search_results = number(value, key, 1, 10000)?
                    }
                    ("search", "search_scan_limit") => {
                        conf.search_limit = number(value, key, 1, 10000000)?
                    }
                    ("colors", _) => {
                        if ![
                            "background",
                            "foreground",
                            "folder_color",
                            "file_color",
                            "selected_color",
                            "line_color",
                            "link_color",
                            "path_color",
                            "mark_color",
                        ]
                        .contains(&key.as_str())
                        {
                            return Err(format!("未知颜色选项: {key}"));
                        }
                        let text = string(value, key)?.trim().to_owned();
                        if !color(&text) {
                            return Err(format!("{key}: 应为 #RRGGBB"));
                        }
                        match key.as_str() {
                            "folder_color" => conf.folder_color = text,
                            "file_color" => conf.file_color = text,
                            "line_color" => conf.line_color = text,
                            "link_color" => conf.link_color = text,
                            "path_color" => conf.path_color = text,
                            "mark_color" => conf.mark_color = text,
                            _ => {}
                        }
                    }
                    ("keys", _) => {
                        if !conf.keys.contains_key(key) {
                            return Err(format!("未知动作: {key}"));
                        }
                        conf.keys.insert(key.clone(), string(value, key)?);
                    }
                    ("openers", _) => {
                        let item = value.as_table().ok_or("程序应为表")?;
                        let mut opener = conf.openers.get(key).cloned().unwrap_or(Opener {
                            label: key.clone(),
                            command: vec![],
                            terminal: false,
                            copy_path: false,
                            pause_on_exit: false,
                            extensions: vec![],
                        });
                        for (field, v) in item {
                            match field.as_str() {
                                "label" => opener.label = string(v, field)?,
                                "terminal" => {
                                    opener.terminal = v.as_bool().ok_or("terminal 应为布尔值")?
                                }
                                "pause_on_exit" => {
                                    opener.pause_on_exit =
                                        v.as_bool().ok_or("pause_on_exit 应为布尔值")?;
                                }
                                "copy_path" => {
                                    opener.copy_path = v.as_bool().ok_or("copy_path 应为布尔值")?;
                                }
                                "extensions" => {
                                    opener.extensions = v
                                        .as_array()
                                        .ok_or("extensions must be an array")?
                                        .iter()
                                        .map(|x| {
                                            let extension = string(x, "extension")?.to_lowercase();
                                            if !extension.starts_with('.')
                                                || extension.len() < 2
                                                || extension.contains(['/', '\\', '\0'])
                                            {
                                                return Err("Extension must be .suffix".into());
                                            }
                                            Ok(extension)
                                        })
                                        .collect::<Result<_, String>>()?;
                                }
                                "command" => {
                                    opener.command = v
                                        .as_array()
                                        .ok_or("command 应为参数列表")?
                                        .iter()
                                        .map(|v| string(v, "command"))
                                        .collect::<Result<_, _>>()?
                                }
                                _ => return Err(format!("未知程序字段: {field}")),
                            }
                        }
                        if opener.command.is_empty()
                            || opener
                                .command
                                .iter()
                                .any(|v| v.is_empty() || v.contains('\0'))
                            || !opener.command.iter().any(|v| v.contains("{path}"))
                            || opener.label.is_empty()
                        {
                            return Err(format!(
                                "openers.{key}: 需要非空参数列表、{{path}} 和 label"
                            ));
                        }
                        conf.openers.insert(key.clone(), opener);
                    }
                    ("folder_colors", _) => {
                        if key.is_empty()
                            || key.contains('/')
                            || [".", ".."].contains(&key.as_str())
                        {
                            return Err(format!("folder_colors.{key}: 应为目录名，不含路径"));
                        }
                        let text = string(value, key)?.trim().to_owned();
                        if !color(&text) {
                            return Err(format!("folder_colors.{key}: 应为 #RRGGBB"));
                        }
                        conf.folder_colors.insert(key.clone(), text);
                    }
                    ("open_defaults" | "extension_colors", _) => {
                        if !["*", "", "<directory>"].contains(&key.as_str())
                            && (!key.starts_with('.') || key.contains('/'))
                        {
                            return Err(format!("扩展名应以 . 开始: {key}"));
                        }
                        let text = string(value, key)?;
                        if section == "extension_colors" {
                            let text = text.trim().to_owned();
                            if !color(&text) {
                                return Err(format!("{key}: 应为 #RRGGBB"));
                            }
                            conf.extension_colors.insert(key.to_lowercase(), text);
                        } else {
                            conf.defaults.insert(key.to_lowercase(), text);
                        }
                    }
                    _ => return Err(format!("未知配置: {section}.{key}")),
                }
            }
        }
        if table
            .get("keys")
            .and_then(|v| v.get("quit"))
            .and_then(Value::as_str)
            == Some("q")
            && table.get("keys").and_then(|v| v.get("dashboard")).is_none()
        {
            conf.keys.insert("quit".into(), "Q".into());
        }
        if conf.keys.get("refresh").map(String::as_str) == Some("r") {
            conf.keys.insert("refresh".into(), "Ctrl+r".into());
        }
        if conf.keys.get("rename").map(String::as_str) == Some("R") {
            conf.keys.insert("rename".into(), "r".into());
        }
        // Newly added defaults yield to existing custom bindings.
        for (action, alternatives) in [
            ("exit_here", &["z", "b", "t", "s", "Ctrl+Alt+x"][..]),
            ("link_info", &["i", "B", "T", "Ctrl+Alt+i"][..]),
            ("dashboard_narrow", &["{", "-", "Ctrl+Alt+b"][..]),
            ("dashboard_widen", &["}", "+", "Ctrl+Alt+w"][..]),
            ("run_foreground", &["Ctrl+Alt+j"][..]),
            ("temporary_focus", &["Ctrl+Alt+f"][..]),
            ("toggle_grouped", &["V", "Ctrl+Alt+v"][..]),
            ("reorder", &["Ctrl+Alt+o"][..]),
            ("hide_item", &["Ctrl+Alt+h"][..]),
            ("hidden_items", &["Ctrl+Alt+z"][..]),
        ] {
            if table
                .get("keys")
                .and_then(|keys| keys.get(action))
                .is_none()
            {
                let occupied: std::collections::HashSet<_> = conf
                    .keys
                    .iter()
                    .filter(|(name, _)| name.as_str() != action)
                    .map(|(_, key)| key_code(key))
                    .collect();
                if occupied.contains(&key_code(&conf.keys[action]))
                    && let Some(key) = alternatives
                        .iter()
                        .find(|key| !occupied.contains(&key_code(key)))
                {
                    conf.keys.insert(action.into(), (*key).into());
                }
            }
        }
        let mut used = std::collections::HashSet::new();
        if conf.keys.get("parent").map(String::as_str) == Some("u") {
            conf.keys.insert("parent".into(), "Left".into());
        }
        for (action, key) in &conf.keys {
            let code = key_code(key);
            let named_control =
                ["ctrl+alt+", "ctrl+shift+", "ctrl+alt+shift+"]
                    .iter()
                    .any(|prefix| {
                        code.len() == prefix.len() + 1
                            && code.starts_with(prefix)
                            && code.as_bytes()[prefix.len()].is_ascii_alphabetic()
                    });
            if (code.chars().count() != 1
                && !(action == "parent" && code == "left")
                && !(action == "refresh" && code == "ctrl+f5")
                && !named_control)
                || (code == *key && key.chars().any(char::is_control))
                || [
                    "\n", "\r", " ", "\x1b", "\x03", "\x15", "\x13", "\x11", "\x1a", "H", "L", ":",
                ]
                .contains(&code.as_str())
            {
                return Err(format!("无效快捷键: {action}={key}"));
            }
            if ["\x06", "/"].contains(&code.as_str()) && action != "search"
                || code == "\x0c" && action != "goto"
                || code == "left" && action != "parent"
                || code == "R" && action != "rename"
                || code == "ctrl+alt+c" && action != "copy_path"
                || code == "\x0e" && action != "new_file"
                || code == "ctrl+shift+n" && action != "new_folder"
                || code == "\t" && action != "switch_pane"
            {
                return Err(
                    "Ctrl+F / Ctrl+L / Shift+R / Ctrl+Alt+C 保留给搜索 / 路径 / 重命名 / 复制路径"
                        .into(),
                );
            }
            if !used.insert(code) {
                return Err("快捷键冲突".into());
            }
        }
        for method in conf.defaults.values() {
            if !conf.openers.contains_key(method) {
                return Err(format!("打开程序未定义: {method}"));
            }
        }
        if conf.cli_opener != "auto"
            && !conf
                .openers
                .get(&conf.cli_opener)
                .is_some_and(|opener| opener.terminal)
        {
            return Err("terminal.cli_opener 必须为 auto 或 terminal=true 的 opener 名称".into());
        }
        Ok(conf)
    }
    pub fn action(&self, key: &str) -> Option<&str> {
        if key == "ctrl+f5" {
            return Some("refresh");
        }
        if key == "\x0e" {
            return Some("new_file");
        }
        if key == "ctrl+shift+n" {
            return Some("new_folder");
        }
        if key == "R" {
            return Some("rename");
        }
        if key == "ctrl+alt+c" {
            return Some("copy_path");
        }
        self.keys
            .iter()
            .find(|(_, v)| key_code(v) == key)
            .map(|(a, _)| a.as_str())
            .or_else(|| (key == "ctrl+alt+n").then_some("new_folder"))
    }
    pub fn rule<'a>(
        &self,
        path: &Path,
        rules: &'a BTreeMap<String, String>,
        dir: bool,
    ) -> Option<&'a String> {
        if dir {
            return rules.get("<directory>").or_else(|| rules.get("*"));
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        rules
            .iter()
            .filter(|(key, _)| key.starts_with('.') && name.ends_with(key.as_str()))
            .max_by_key(|(key, _)| key.len())
            .map(|(_, v)| v)
            .or_else(|| {
                if path.extension().is_none() {
                    rules.get("").or_else(|| rules.get("*"))
                } else {
                    rules.get("*")
                }
            })
    }
}
