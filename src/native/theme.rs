//! Shared product tokens. Native code only resolves the current macOS appearance.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Palette {
    paper: [u8; 3],
    surface: [u8; 3],
    ink: [u8; 3],
    sub: [u8; 3],
    rule: [u8; 3],
    selection: [u8; 3],
    accent: [u8; 3],
    accent_soft: [u8; 3],
    on_accent: [u8; 3],
    error: [u8; 3],
}
#[derive(Deserialize)]
pub(super) struct Radius {
    pub window: f64,
    pub card: f64,
    pub control: f64,
}
#[derive(Deserialize)]
pub(super) struct Space {
    pub inset: f64,
    pub gap: f64,
}
#[derive(Deserialize)]
pub(super) struct Type {
    pub title: f64,
    pub body: f64,
    pub caption: f64,
}
#[derive(Deserialize)]
pub(super) struct Switcher {
    pub tile: f64,
    pub icon: f64,
    pub height: f64,
}
#[derive(Deserialize)]
pub(super) struct Tokens {
    light: Palette,
    dark: Palette,
    pub radius: Radius,
    pub space: Space,
    #[serde(rename = "type")]
    pub text: Type,
    pub switcher: Switcher,
}
#[derive(Clone, Copy)]
pub(super) enum Role {
    Paper,
    Surface,
    Ink,
    Sub,
    Rule,
    Selection,
    Accent,
    AccentSoft,
    OnAccent,
    Error,
}
pub(super) fn tokens() -> &'static Tokens {
    static TOKENS: OnceLock<Tokens> = OnceLock::new();
    TOKENS.get_or_init(|| {
        serde_json::from_str(include_str!("../../design/tokens.json"))
            .expect("Valid Tafo design tokens")
    })
}
pub(super) unsafe fn is_dark() -> bool {
    let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
    let appearance: *mut AnyObject = msg_send![app, effectiveAppearance];
    string(msg_send![appearance, name]).contains("Dark")
}
pub(super) unsafe fn color(role: Role) -> *mut AnyObject {
    let design = tokens();
    let p = if is_dark() {
        &design.dark
    } else {
        &design.light
    };
    let [r, g, b] = match role {
        Role::Paper => p.paper,
        Role::Surface => p.surface,
        Role::Ink => p.ink,
        Role::Sub => p.sub,
        Role::Rule => p.rule,
        Role::Selection => p.selection,
        Role::Accent => p.accent,
        Role::AccentSoft => p.accent_soft,
        Role::OnAccent => p.on_accent,
        Role::Error => p.error,
    };
    super::color(r as f64 / 255., g as f64 / 255., b as f64 / 255.)
}
pub(super) unsafe fn surface(view: *mut AnyObject, radius: f64) {
    rounded(view, radius, color(Role::Paper));
    let layer: *mut AnyObject = msg_send![view, layer];
    let border: *const c_void = msg_send![color(Role::Rule), CGColor];
    let _: () = msg_send![layer,setBorderWidth:if radius>0.{0.75f64}else{0.0f64}];
    let _: () = msg_send![layer,setBorderColor:border];
}
