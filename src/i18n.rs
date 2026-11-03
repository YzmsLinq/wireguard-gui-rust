//! Interface language support.
//!
//! The Rust version keeps the behaviour of the Python one: the language can be
//! changed at any time from the `Set I/F|Lang.` menu, and the choice is stored
//! in `~/.config/wireguard-gui-rust/wgr.ini` so it survives a restart.
//!
//! Adding a language only requires adding an entry to [`SUPPORTED_LANGUAGES`],
//! a [`Lang`] variant and the matching column in the `messages!` table below.

use crate::settings;

/// Pseudo language meaning "use the language of the system".
pub const SYSTEM_LANGUAGE: &str = "system";

/// Language used when the system language is not supported.
pub const DEFAULT_LANGUAGE: &str = "en";

/// Language code -> name displayed to the user, written in its own language.
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[("en", "English"), ("zh_CN", "中文")];

const SETTINGS_SECTION: &str = "interface";
const SETTINGS_KEY: &str = "language";

/// A language the interface can be displayed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    ZhCn,
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

macro_rules! messages {
    ($( $variant:ident => ($english:expr, $chinese:expr) ),* $(,)?) => {
        /// Every string displayed by the application.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Msg {
            $($variant),*
        }

        impl Msg {
            pub fn text(self, lang: Lang) -> &'static str {
                match (self, lang) {
                    $(
                        (Msg::$variant, Lang::En) => $english,
                        (Msg::$variant, Lang::ZhCn) => $chinese,
                    )*
                }
            }
        }
    };
}

messages! {
    AppTitle => ("wgr", "wgr"),
    // Main window.
    Configs => ("Configs", "配置"),
    Logs => ("Logs", "日志"),
    Interface => ("Interface", "接口"),
    Peers => ("Peers", "节点"),
    Export => ("Export", "导出"),
    Delete => ("Delete", "删除"),
    Edit => ("Edit", "编辑"),
    SetIfLang => ("Set I/F|Lang.", "设置接口|语言"),
    ActivateDeactivate => ("Activate / Deactivate", "连接 / 断开"),
    Deactivate => ("Deactivate", "断开"),
    InterfacePrefix => ("Interface: ", "接口： "),
    // Menus.
    NewInterface => ("New interface", "新建接口"),
    AddInterfaceFile => ("Add interface file", "添加接口文件"),
    ImportInterfaces => ("Import interfaces", "导入接口"),
    Language => ("Language", "语言"),
    SystemDefault => ("System default", "系统默认"),
    Show => ("Show", "显示主窗口"),
    Quit => ("Quit", "退出"),
    // Messages.
    Error => ("Error", "错误"),
    Yes => ("Yes", "是"),
    No => ("No", "否"),
    Ok => ("OK", "确定"),
    Cancel => ("Cancel", "取消"),
    SelectInterface => ("You have to select an interface", "你必须选择一个接口"),
    ConfigUpdated => (
        "Configuration updated successfully",
        "成功更新配置"
    ),
    ConfigInvalidRollback => (
        "Invalid configuration, returning to the previous configuration",
        "配置无效，正在恢复至之前的配置"
    ),
    ConfirmDeleting => ("Confirm deleting", "确认删除"),
    ConfirmDeletePrefix => ("Are you sure you want to delete ", "你确定要删除 "),
    ConfirmDeleteSuffix => (" interfaces?", " 接口？"),
    SelectDirectory => ("Select directory", "选择目录"),
    SelectConfigFile => ("Select config file", "选择配置文件"),
    SelectZipFile => ("Select zip file", "选择 zip 文件"),
    Exported => ("Interfaces exported successfully", "成功导出接口"),
    InterfaceAdded => ("Interface added successfully", "成功新增接口"),
    Imported => ("Interfaces imported successfully", "成功导入接口"),
    AddedSuccessfullySuffix => (" added successfully", " 增加成功"),
    TrayTooltip => ("WireGuard client", "WireGuard 客户端"),
    // Dialogs.
    NewDialogTitle => ("New interface", "新建接口"),
    EditDialogTitle => ("Edit", "编辑"),
    InterfaceName => ("Interface name", "接口名称"),
    ConfSuffix => (".conf", ".conf"),
    PublicKey => ("Public key", "公钥"),
    Configuration => ("Configuration", "配置"),
    EmptyName => ("The interface name cannot be empty", "接口名称不能为空"),
    EmptyConfiguration => ("The configuration cannot be empty", "配置内容不能为空"),
    // File picker.
    Up => ("Up", "上一级"),
    Open => ("Open", "打开"),
    EmptyFolder => ("This folder is empty", "此目录为空"),
    NotADirectory => ("This path is not a folder", "该路径不是目录"),
    // Configuration validation.
    ErrNoInterface => (
        "No [Interface] section found",
        "未找到 [Interface] 段"
    ),
    ErrNoPrivateKey => (
        "The [Interface] section has no PrivateKey",
        "[Interface] 段缺少 PrivateKey"
    ),
    ErrPeerNoPublicKey => (
        "A [Peer] section has no PublicKey",
        "存在缺少 PublicKey 的 [Peer] 段"
    ),
    // Root check.
    RootRequired => ("You need root privileges", "你需要 root 权限"),
    RootRequiredDetail => (
        "wgr manages /etc/wireguard and the wg-quick services, it has to run as root.",
        "wgr 需要管理 /etc/wireguard 与 wg-quick 服务，必须以 root 身份运行。"
    ),
    RootRequiredManual => (
        "Start wgr from a terminal with:",
        "可以在终端中用以下命令启动："
    ),
    NoElevationTool => (
        "Neither pkexec nor sudo is available, wgr cannot ask for the elevation itself.",
        "系统中没有 pkexec 或 sudo，wgr 无法自行请求提权。"
    ),
    ElevationCancelled => (
        "The elevation was refused or cancelled.",
        "提权被拒绝或已取消。"
    ),
    Copy => ("Copy", "复制"),
    Close => ("Close", "关闭"),
    // Fonts.
    MissingCjkFont => (
        "No CJK font found, install one to display Chinese correctly",
        "未找到中文字体，请安装字体以正确显示中文"
    ),
    ActiveCount => ("Active interfaces: ", "已连接接口："),
}

/// The string `message` in `lang`.
pub fn t(message: Msg, lang: Lang) -> &'static str {
    message.text(lang)
}

/// `(language code, name shown in the menu)` pairs, system language first.
pub fn choices() -> Vec<(&'static str, Option<&'static str>)> {
    let mut languages = vec![(SYSTEM_LANGUAGE, None)];
    languages.extend(SUPPORTED_LANGUAGES.iter().map(|(code, name)| (*code, Some(*name))));
    languages
}

fn code_to_lang(code: &str) -> Option<Lang> {
    SUPPORTED_LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(candidate, _)| match *candidate {
            "zh_CN" => Lang::ZhCn,
            _ => Lang::En,
        })
}

/// Resolves the locale of the system to a supported language.
pub fn detect_system_language() -> Lang {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|variable| std::env::var(variable).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_default();

    let locale = locale.split('.').next().unwrap_or("").to_owned();

    if let Some(lang) = code_to_lang(&locale) {
        return lang;
    }

    let short = locale.split('_').next().unwrap_or("").to_ascii_lowercase();

    for (code, _) in SUPPORTED_LANGUAGES {
        if code.split('_').next().unwrap_or("").eq_ignore_ascii_case(&short) {
            if let Some(lang) = code_to_lang(code) {
                return lang;
            }
        }
    }

    code_to_lang(DEFAULT_LANGUAGE).unwrap_or(Lang::En)
}

/// Turns a stored preference into the language to display.
pub fn resolve(preference: &str) -> Lang {
    if preference == SYSTEM_LANGUAGE {
        return detect_system_language();
    }

    code_to_lang(preference).unwrap_or_else(|| code_to_lang(DEFAULT_LANGUAGE).unwrap_or(Lang::En))
}

/// The language preference stored by the user (`system` by default).
pub fn saved_preference() -> String {
    match settings::get(SETTINGS_SECTION, SETTINGS_KEY) {
        Some(value) if value == SYSTEM_LANGUAGE || code_to_lang(&value).is_some() => value,
        _ => SYSTEM_LANGUAGE.to_owned(),
    }
}

/// Stores the language chosen by the user.
pub fn save_preference(preference: &str) {
    settings::set(SETTINGS_SECTION, SETTINGS_KEY, preference);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_is_translated() {
        for (code, _) in SUPPORTED_LANGUAGES {
            let lang = code_to_lang(code).expect("supported language");
            assert!(!Msg::Export.text(lang).is_empty());
            assert!(!Msg::RootRequired.text(lang).is_empty());
        }
    }

    #[test]
    fn falls_back_to_the_default_language() {
        assert_eq!(resolve("xx_YY"), Lang::En);
        assert_eq!(resolve("zh_CN"), Lang::ZhCn);
    }
}
