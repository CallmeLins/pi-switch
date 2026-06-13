use std::sync::OnceLock;
use std::sync::RwLock;

use crate::config::{load_config, save_config, PiSwitchConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Zh,
}

fn lang_store() -> &'static RwLock<Lang> {
    static STORE: OnceLock<RwLock<Lang>> = OnceLock::new();
    STORE.get_or_init(|| {
        let lang = detect_lang_from_config();
        RwLock::new(lang)
    })
}

fn detect_lang_from_config() -> Lang {
    load_config()
        .ok()
        .and_then(|c: PiSwitchConfig| c.settings.language)
        .map(|s| lang_from_code(&s))
        .unwrap_or_else(|| {
            // Fallback to env var, then English
            match std::env::var("PI_SWITCH_LANG")
                .unwrap_or_default()
                .to_lowercase()
                .as_str()
            {
                "zh" | "zh-cn" | "zh-tw" | "chinese" => Lang::Zh,
                _ => Lang::En,
            }
        })
}

fn lang_from_code(code: &str) -> Lang {
    match code.to_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-tw" | "chinese" => Lang::Zh,
        _ => Lang::En,
    }
}

pub fn current_lang() -> Lang {
    *lang_store().read().expect("Failed to read language")
}

pub fn is_zh() -> bool {
    current_lang() == Lang::Zh
}

/// Set language and persist to config.
pub fn set_lang(lang: Lang) -> Result<(), String> {
    {
        let mut guard = lang_store().write().expect("Failed to write language");
        *guard = lang;
    }

    // Persist to config
    let mut config = load_config().map_err(|e| e.to_string())?;
    config.settings.language = Some(lang.code().to_string());
    save_config(&config).map_err(|e| e.to_string())?;

    Ok(())
}

impl Lang {
    pub fn code(&self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Zh => "zh",
        }
    }
}

#[macro_export]
macro_rules! t {
    ($en:expr, $zh:expr) => {
        if $crate::tui::i18n::is_zh() { $zh } else { $en }
    };
}

// ─── Navigation ──────────────────────────────────────────

pub fn nav_home() -> &'static str { t!("Home", "首页") }
pub fn nav_profiles() -> &'static str { t!("Profiles", "供应商") }
pub fn nav_presets() -> &'static str { t!("Presets", "模板") }
pub fn nav_proxy() -> &'static str { t!("Proxy", "代理") }
pub fn nav_stats() -> &'static str { t!("Stats", "统计") }
pub fn nav_backups() -> &'static str { t!("Backups", "备份") }
pub fn nav_settings() -> &'static str { t!("Settings", "设置") }
pub fn nav_exit() -> &'static str { t!("Exit", "退出") }

// ─── Header ──────────────────────────────────────────────

pub fn header_title() -> &'static str { "  pi-switch" }
pub fn header_proxy_running(port: u16) -> String {
    if is_zh() { format!("  代理 ●  :{port}  ") } else { format!("  Proxy ●  :{port}  ") }
}
pub fn header_provider(current: &str) -> String {
    if is_zh() { format!("  当前供应商: {current}  ") } else { format!("  Provider: {current}  ") }
}

// ─── Footer ──────────────────────────────────────────────

pub fn footer_filter() -> &'static str {
    t!(" Filter: type to search   Enter confirm   Esc clear", " 过滤: 输入搜索  Enter 确认  Esc 清除")
}
pub fn footer_no_color() -> &'static str {
    t!(" <->: menu/content  ^v: move  /: filter  Esc: back  ?: help  q: quit",
       " ←→: 菜单/内容  ↑↓: 移动  /: 过滤  Esc: 返回  ?: 帮助  q: 退出")
}
pub fn footer_nav_label() -> &'static str { t!("menu/content", "菜单/内容") }
pub fn footer_nav_move() -> &'static str { t!("move", "移动") }
pub fn footer_act_filter() -> &'static str { t!("filter", "过滤") }
pub fn footer_act_back() -> &'static str { t!("back", "返回") }
pub fn footer_act_help() -> &'static str { t!("help", "帮助") }
pub fn footer_act_quit() -> &'static str { t!("quit", "退出") }

// ─── Pages ───────────────────────────────────────────────

pub fn page_home() -> &'static str { t!(" Home ", " 首页 ") }
pub fn page_presets() -> &'static str { t!(" Presets ", " 模板 ") }
pub fn page_proxy() -> &'static str { t!(" Proxy ", " 代理 ") }
pub fn page_stats() -> &'static str { t!(" Stats ", " 统计 ") }
pub fn page_backups_count(n: usize) -> String {
    if is_zh() { format!(" 备份 ({n}) ") } else { format!(" Backups ({n}) ") }
}
pub fn page_settings() -> &'static str { t!(" Settings ", " 设置 ") }
pub fn page_profiles(total: usize) -> String {
    if is_zh() { format!(" 供应商 ({total}) ") } else { format!(" Profiles ({total}) ") }
}
pub fn page_profiles_filtered(visible: usize, total: usize) -> String {
    if is_zh() { format!(" 供应商 ({visible}/{total}) ") } else { format!(" Profiles ({visible}/{total}) ") }
}

// ─── Home labels ─────────────────────────────────────────

pub fn home_profiles() -> &'static str { t!("Profiles", "供应商") }
pub fn home_current() -> &'static str { t!("Current", "当前") }
pub fn home_write_mode() -> &'static str { t!("Write mode", "写入模式") }
pub fn home_provider_prefix() -> &'static str { t!("Provider prefix", "供应商前缀") }
pub fn home_proxy_daemon() -> &'static str { t!("Proxy daemon", "代理守护进程") }
pub fn home_running(pid: u32, host: &str, port: u16) -> String {
    if is_zh() { format!("运行中 (PID {pid}) 于 {host}:{port}") } else { format!("running (PID {pid}) on {host}:{port}") }
}
pub fn home_stopped() -> &'static str { t!("stopped", "已停止") }
pub fn home_config() -> &'static str { t!("Config", "配置") }
pub fn home_pi_models() -> &'static str { t!("Pi models", "Pi 模型") }
pub fn home_backups() -> &'static str { t!("Backups", "备份") }
pub fn home_requests() -> &'static str { t!("Requests", "请求") }
pub fn home_requests_fmt(total: u64, ok: u64, rate: &str) -> String {
    if is_zh() { format!("{total} 总计, {ok} 成功 ({rate})") } else { format!("{total} total, {ok} ok ({rate})") }
}

// ─── Proxy labels ────────────────────────────────────────

pub fn proxy_target() -> &'static str { t!("Target", "目标") }
pub fn proxy_failover() -> &'static str { t!("Failover", "故障转移") }
pub fn proxy_listen() -> &'static str { t!("Listen", "监听") }
pub fn proxy_running(pid: u32) -> String {
    if is_zh() { format!("  ● 运行中 (PID {pid})") } else { format!("  ● RUNNING (PID {pid})") }
}
pub fn proxy_stopped() -> &'static str { t!("  ○ Stopped", "  ○ 已停止") }

// ─── Stats labels ────────────────────────────────────────

pub fn stats_no_data() -> &'static str {
    t!("  No request data. Start the proxy and make some requests first.",
       "  无请求数据。请先启动代理并发送一些请求。")
}
pub fn stats_requests_fmt(total: u64, ok: u64, failed: u64, rate: &str) -> String {
    if is_zh() { format!("{total} 总计 | {ok} 成功 | {failed} 失败 | {rate}") }
    else { format!("{total} total | {ok} ok | {failed} failed | {rate}") }
}
pub fn stats_avg_latency() -> &'static str { t!("Avg latency", "平均延迟") }
pub fn stats_retries_skipped() -> &'static str { t!("Retries / skipped", "重试 / 跳过") }
pub fn stats_by_provider() -> &'static str { t!("  By provider", "  按供应商") }
pub fn stats_by_model() -> &'static str { t!("  By model", "  按模型") }

// ─── Backups labels ──────────────────────────────────────

pub fn backups_empty() -> &'static str {
    t!("  No backups yet.", "  暂无备份。")
}

// ─── Profiles table ──────────────────────────────────────

pub fn profiles_api_url_col() -> &'static str { t!("API URL", "API 地址") }
pub fn profiles_name_id_col() -> &'static str { t!("Name / ID", "名称 / ID") }
pub fn profiles_marker_current() -> &'static str { t!("*", "★") }
pub fn profiles_marker_other() -> &'static str { " " }
pub fn profiles_proxy_badge() -> &'static str { t!("[proxy]", "[代理]") }
pub fn profiles_empty_add() -> &'static str {
    t!("No profiles yet.\n\nPress a to add your first profile,\nor pick a template from Presets.",
       "暂无供应商。\n\n按 a 添加第一个供应商，\n或从模板中导入。")
}
pub fn profiles_no_match() -> &'static str {
    t!("No profiles match the filter.\n\nPress Esc to clear the filter.",
       "没有匹配的供应商。\n\n按 Esc 清除过滤。")
}

// ─── Profile detail ──────────────────────────────────────

pub fn detail_title(name: &str) -> String {
    if is_zh() { format!(" 供应商详情: {name} ") } else { format!(" Profile: {name} ") }
}
pub fn detail_name() -> &'static str { t!("Name", "名称") }
pub fn detail_current() -> &'static str { t!("Current", "当前") }
pub fn detail_current_yes() -> &'static str { t!("yes ★", "是 ★") }
pub fn detail_current_no() -> &'static str { t!("no", "否") }
pub fn detail_provider_id() -> &'static str { t!("Provider ID", "供应商 ID") }
pub fn detail_api() -> &'static str { t!("API", "API") }
pub fn detail_base_url() -> &'static str { t!("Base URL", "API 地址") }
pub fn detail_api_key() -> &'static str { t!("API Key", "API 密钥") }
pub fn detail_preset() -> &'static str { t!("Preset", "模板") }
pub fn detail_updated() -> &'static str { t!("Updated", "更新时间") }
pub fn detail_models() -> &'static str { t!("  Models", "  模型") }
pub fn detail_not_found() -> &'static str {
    t!("Profile not found.", "供应商未找到。")
}

// ─── Form ────────────────────────────────────────────────

pub fn form_add_title() -> &'static str { t!(" Add Profile ", " 添加供应商 ") }
pub fn form_edit_title(name: &str) -> String {
    if is_zh() { format!(" 编辑供应商: {name} ") } else { format!(" Edit Profile: {name} ") }
}
pub fn form_template_pane() -> &'static str { t!(" Template ", " 模板 ") }
pub fn form_custom_chip() -> &'static str { t!("Custom", "自定义") }
pub fn form_fields_pane() -> &'static str { t!(" Fields ", " 字段 ") }
pub fn form_json_preview_pane() -> &'static str { t!(" JSON Preview ", " JSON 预览 ") }
pub fn form_editing(field: &str) -> String {
    if is_zh() { format!(" {field} (编辑中) ") } else { format!(" {field} (editing) ") }
}
pub fn form_field_title(field: &str) -> String {
    format!(" {field} ")
}
pub fn form_api_cycle_hint() -> &'static str {
    t!("   (Space/←→ cycle)", "   (空格/←→ 切换)")
}

// ─── Key bar hints ───────────────────────────────────────

pub fn key_detail() -> &'static str { t!("detail", "详情") }
pub fn key_switch() -> &'static str { t!("switch", "切换") }
pub fn key_add() -> &'static str { t!("add", "添加") }
pub fn key_copy() -> &'static str { t!("copy", "复制") }
pub fn key_edit() -> &'static str { t!("edit", "编辑") }
pub fn key_delete() -> &'static str { t!("delete", "删除") }
pub fn key_filter() -> &'static str { t!("filter", "过滤") }
pub fn key_refresh() -> &'static str { t!("refresh", "刷新") }
pub fn key_move() -> &'static str { t!("move", "移动") }
pub fn key_back() -> &'static str { t!("back", "返回") }
pub fn key_scroll() -> &'static str { t!("scroll", "滚动") }
pub fn key_use_template() -> &'static str { t!("use as template", "作为模板") }
pub fn key_run_action() -> &'static str { t!("run action", "运行") }
pub fn key_focus() -> &'static str { t!("focus", "焦点") }
pub fn key_field() -> &'static str { t!("field", "字段") }
pub fn key_edit_apply() -> &'static str { t!("edit/apply", "编辑/应用") }
pub fn key_save() -> &'static str { t!("save", "保存") }
pub fn key_close() -> &'static str { t!("close", "关闭") }
pub fn key_confirm() -> &'static str { t!("confirm", "确认") }
pub fn key_cancel() -> &'static str { t!("cancel", "取消") }

// ─── Overlay: Help ───────────────────────────────────────

pub fn help_title() -> &'static str { t!(" Help ", " 帮助 ") }
pub fn help_section_global() -> &'static str { t!("  Global", "  全局") }
pub fn help_arrow_keys() -> &'static str {
    t!("    ←→ / h l       switch between menu and content",
       "    ←→ / h l       切换菜单/内容焦点")
}
pub fn help_updown_keys() -> &'static str {
    t!("    ↑↓ / j k        move selection", "    ↑↓ / j k        移动选择")
}
pub fn help_enter() -> &'static str {
    t!("    Enter           open / confirm", "    Enter           打开 / 确认")
}
pub fn help_esc() -> &'static str {
    t!("    Esc             back / clear filter", "    Esc             返回 / 清除过滤")
}
pub fn help_q() -> &'static str {
    t!("    q               quit (with confirmation)", "    q               退出 (需确认)")
}
pub fn help_ctrl_c() -> &'static str {
    t!("    Ctrl+C          quit immediately", "    Ctrl+C          立即退出")
}
pub fn help_question() -> &'static str {
    t!("    ?               toggle this help", "    ?               显示/关闭帮助")
}
pub fn help_section_profiles() -> &'static str { t!("  Profiles", "  供应商") }
pub fn help_profiles_enter() -> &'static str {
    t!("    Enter           open profile detail", "    Enter           打开供应商详情")
}
pub fn help_profiles_space() -> &'static str {
    t!("    Space / s       switch pi to this profile", "    Space / s       切换 pi 到此供应商")
}
pub fn help_profiles_a() -> &'static str {
    t!("    a               add profile", "    a               添加供应商")
}
pub fn help_profiles_e() -> &'static str {
    t!("    e               edit profile", "    e               编辑供应商")
}
pub fn help_profiles_c() -> &'static str {
    t!("    c               copy profile", "    c               复制供应商")
}
pub fn help_profiles_d() -> &'static str {
    t!("    d               delete profile (confirm)", "    d               删除供应商 (需确认)")
}
pub fn help_profiles_slash() -> &'static str {
    t!("    /               filter list", "    /               过滤列表")
}
pub fn help_profiles_r() -> &'static str {
    t!("    r               refresh data", "    r               刷新数据")
}
pub fn help_section_form() -> &'static str { t!("  Form", "  表单") }
pub fn help_form_tab() -> &'static str {
    t!("    Tab             cycle focus (template / fields / preview)",
       "    Tab             切换焦点 (模板 / 字段 / 预览)")
}
pub fn help_form_enter() -> &'static str {
    t!("    Enter           edit field / apply template", "    Enter           编辑字段 / 应用模板")
}
pub fn help_form_space() -> &'static str {
    t!("    Space           cycle API type", "    Space           切换 API 类型")
}
pub fn help_form_ctrl_s() -> &'static str {
    t!("    Ctrl+S          save profile", "    Ctrl+S          保存供应商")
}
pub fn help_form_esc() -> &'static str {
    t!("    Esc             close (asks when unsaved)", "    Esc             关闭 (未保存时询问)")
}
pub fn help_section_editing() -> &'static str { t!("  Text editing", "  文本编辑") }
pub fn help_edit_ctrl_ae() -> &'static str {
    t!("    Ctrl+A / E      line start / end", "    Ctrl+A / E      行首 / 行尾")
}
pub fn help_edit_ctrl_bf() -> &'static str {
    t!("    Ctrl+B / F      move left / right", "    Ctrl+B / F      左移 / 右移")
}
pub fn help_edit_alt_bf() -> &'static str {
    t!("    Alt+B / F       move by word", "    Alt+B / F       按词移动")
}
pub fn help_edit_ctrl_w() -> &'static str {
    t!("    Ctrl+W          delete word backward", "    Ctrl+W          删除前词")
}
pub fn help_edit_ctrl_uk() -> &'static str {
    t!("    Ctrl+U / K      delete to line start / end", "    Ctrl+U / K      删除至行首 / 行尾")
}
pub fn help_section_presets() -> &'static str { t!("  Presets", "  模板") }
pub fn help_presets_enter() -> &'static str {
    t!("    Enter           create profile from preset", "    Enter           从模板创建供应商")
}

// ─── Overlay: Confirm ────────────────────────────────────

pub fn confirm_exit_title() -> &'static str { t!("Exit", "退出") }
pub fn confirm_exit_msg() -> &'static str { t!("Quit pi-switch?", "退出 pi-switch?") }
pub fn confirm_delete_title() -> &'static str { t!("Delete profile", "删除供应商") }
pub fn confirm_delete_msg(name: &str) -> String {
    if is_zh() { format!("删除供应商 '{name}'? 此操作不可撤销。") }
    else { format!("Delete profile '{name}'? This cannot be undone.") }
}
pub fn confirm_discard_title() -> &'static str { t!("Discard Changes", "放弃修改") }
pub fn confirm_discard_msg() -> &'static str {
    t!("You have unsaved changes.\nDiscard them?", "有未保存的修改。\n确定放弃？")
}

// ─── Toast messages ──────────────────────────────────────

pub fn toast_refreshed() -> &'static str { t!("Refreshed", "已刷新") }
pub fn toast_switched(name: &str, id: &str) -> String {
    if is_zh() { format!("已切换到 '{name}' ({id})") }
    else { format!("Switched to '{name}' ({id})") }
}
pub fn toast_status_refreshed() -> &'static str { t!("Status refreshed", "状态已刷新") }
pub fn toast_copied(src: &str, dst: &str) -> String {
    if is_zh() { format!("已复制 '{src}' 为 '{dst}'") }
    else { format!("Copied '{src}' to '{dst}'") }
}
pub fn toast_saved(name: &str) -> String {
    if is_zh() { format!("已保存供应商 '{name}'") }
    else { format!("Saved profile '{name}'") }
}
pub fn toast_deleted(name: &str) -> String {
    if is_zh() { format!("已删除供应商 '{name}'") }
    else { format!("Deleted profile '{name}'") }
}
pub fn toast_invalid_json(e: &str) -> String {
    if is_zh() { format!("无效的供应商 JSON: {e}") }
    else { format!("Invalid profile JSON: {e}") }
}
pub fn toast_lang_set(code: &str) -> String {
    if is_zh() { format!("语言已设为: {}", if code == "zh" { "中文" } else { "English" }) }
    else { format!("Language set to: {}", if code == "zh" { "Chinese" } else { "English" }) }
}

// ─── Settings page ───────────────────────────────────────

pub fn settings_lang_label() -> &'static str { t!("Language", "语言") }
pub fn settings_lang_en() -> &'static str { t!("English", "English") }
pub fn settings_lang_zh() -> &'static str { t!("中文", "中文") }
pub fn settings_proxy_host() -> &'static str { t!("Proxy host", "代理地址") }
pub fn settings_proxy_port() -> &'static str { t!("Proxy port", "代理端口") }
pub fn settings_proxy_target() -> &'static str { t!("Proxy target", "代理目标") }
pub fn settings_proxy_failover() -> &'static str { t!("Failover chain", "故障转移链") }
pub fn settings_saved() -> &'static str { t!("Settings saved", "设置已保存") }
pub fn settings_header_setting() -> &'static str { t!("Setting", "设置项") }
pub fn settings_header_value() -> &'static str { t!("Value", "当前值") }

// ─── Proxy actions ───────────────────────────────────────

pub fn proxy_action_start() -> &'static str { t!("Start daemon", "启动守护进程") }
pub fn proxy_action_stop() -> &'static str { t!("Stop daemon", "停止守护进程") }
pub fn proxy_action_status() -> &'static str { t!("Refresh status", "刷新状态") }

// ─── Menu title ──────────────────────────────────────────

pub fn menu_title() -> &'static str { t!(" Menu ", " 菜单 ") }

// ─── Field labels ────────────────────────────────────────

pub fn field_name() -> &'static str { t!("name", "名称") }
pub fn field_api() -> &'static str { t!("api", "API 类型") }
pub fn field_base_url() -> &'static str { t!("baseUrl", "API 地址") }
pub fn field_api_key() -> &'static str { t!("apiKey", "API 密钥") }
pub fn field_models() -> &'static str { t!("models", "模型") }
