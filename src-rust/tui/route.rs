use super::i18n;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    Profiles,
    ProfileDetail(String),
    Presets,
    Proxy,
    Stats,
    Backups,
    Settings,
    Form,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavItem {
    Home,
    Profiles,
    Presets,
    Proxy,
    Stats,
    Backups,
    Settings,
    Exit,
}

impl NavItem {
    pub const ALL: [NavItem; 8] = [
        NavItem::Home,
        NavItem::Profiles,
        NavItem::Presets,
        NavItem::Proxy,
        NavItem::Stats,
        NavItem::Backups,
        NavItem::Settings,
        NavItem::Exit,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            NavItem::Home => i18n::nav_home(),
            NavItem::Profiles => i18n::nav_profiles(),
            NavItem::Presets => i18n::nav_presets(),
            NavItem::Proxy => i18n::nav_proxy(),
            NavItem::Stats => i18n::nav_stats(),
            NavItem::Backups => i18n::nav_backups(),
            NavItem::Settings => i18n::nav_settings(),
            NavItem::Exit => i18n::nav_exit(),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            NavItem::Home => "⌂",
            NavItem::Profiles => "▤",
            NavItem::Presets => "◇",
            NavItem::Proxy => "⇄",
            NavItem::Stats => "∑",
            NavItem::Backups => "⊙",
            NavItem::Settings => "⚙",
            NavItem::Exit => "✕",
        }
    }

    pub fn to_route(&self) -> Option<Route> {
        match self {
            NavItem::Home => Some(Route::Home),
            NavItem::Profiles => Some(Route::Profiles),
            NavItem::Presets => Some(Route::Presets),
            NavItem::Proxy => Some(Route::Proxy),
            NavItem::Stats => Some(Route::Stats),
            NavItem::Backups => Some(Route::Backups),
            NavItem::Settings => Some(Route::Settings),
            NavItem::Exit => None,
        }
    }
}
