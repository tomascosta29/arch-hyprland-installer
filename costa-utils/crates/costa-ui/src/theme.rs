//! Shared soft-glass theme for all Costa Utils overlays.

use adw::prelude::*;
use gtk4::gdk;
use gtk4::{CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::sync::atomic::{AtomicBool, Ordering};

/// Install the application-wide CSS once a display is available.
/// Safe to call multiple times; retries until the provider is attached.
pub fn install() {
    static LOADED: AtomicBool = AtomicBool::new(false);
    if LOADED.load(Ordering::Relaxed) {
        return;
    }
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = CssProvider::new();
    provider.load_from_string(THEME_CSS);
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    LOADED.store(true, Ordering::Relaxed);
}

pub fn style_window(window: &adw::ApplicationWindow) {
    window.add_css_class("costa-window");
}

pub fn header(title: &str, subtitle: &str) -> adw::HeaderBar {
    let header = adw::HeaderBar::new();
    header.add_css_class("costa-header");
    header.set_show_title(true);

    let heading = gtk4::Box::new(gtk4::Orientation::Vertical, 1);
    heading.set_halign(gtk4::Align::Start);
    let title_label = gtk4::Label::new(Some(title));
    title_label.add_css_class("costa-header-title");
    title_label.set_halign(gtk4::Align::Start);
    let subtitle_label = gtk4::Label::new(Some(subtitle));
    subtitle_label.add_css_class("costa-header-subtitle");
    subtitle_label.set_halign(gtk4::Align::Start);
    heading.append(&title_label);
    heading.append(&subtitle_label);
    header.set_title_widget(Some(&heading));
    header
}

const THEME_CSS: &str = r#"
/* —— Popup chrome —— */
window {
    background: @window_bg_color;
    /* Match Hyprland decoration.rounding (dotfiles/hypr/hyprland.lua). */
    border-radius: 8px;
}

window.control-center {
    background: linear-gradient(
        to bottom,
        shade(@window_bg_color, 1.12),
        @window_bg_color 42%,
        shade(@window_bg_color, 0.96)
    );
}

window.costa-window {
    background: linear-gradient(
        to bottom,
        shade(@window_bg_color, 1.1),
        @window_bg_color 38%,
        shade(@window_bg_color, 0.97)
    );
}

headerbar {
    background: transparent;
    box-shadow: none;
}

headerbar windowcontrols {
    margin: 0 4px;
}

.costa-header {
    min-height: 48px;
    padding: 6px 10px 2px 18px;
    background: transparent;
    box-shadow: none;
    border-bottom: 1px solid alpha(@view_fg_color, 0.045);
}

.costa-header-title {
    font-size: 1.08em;
    font-weight: 700;
}

.costa-header-subtitle {
    font-size: 0.72em;
    opacity: 0.42;
}

.costa-header windowcontrols button,
.costa-header > button {
    min-width: 28px;
    min-height: 28px;
    border-radius: 999px;
    background: alpha(@view_fg_color, 0.045);
}

.costa-header windowcontrols button:hover,
.costa-header > button:hover {
    background: alpha(@view_fg_color, 0.1);
}

/* —— Shared surfaces —— */
.card-box {
    background: linear-gradient(
        to bottom,
        mix(@window_bg_color, white, 0.04),
        mix(@window_bg_color, white, 0.022)
    );
    border: 1px solid alpha(@view_fg_color, 0.065);
    border-radius: 11px;
    box-shadow:
        inset 0 1px 0 alpha(white, 0.025),
        0 1px 3px alpha(black, 0.1);
    padding: 12px 14px;
}

.media-card {
    padding: 12px 14px;
}

.password-card {
    background: alpha(@view_fg_color, 0.04);
    border: 1px solid alpha(@view_fg_color, 0.08);
    border-radius: 16px;
    padding: 28px 32px;
    min-width: 320px;
}

.password-title {
    font-size: 1.1em;
    font-weight: 600;
    margin-bottom: 8px;
}

/* —— Typography —— */
.bold-label { font-weight: 600; }
.dim-label { opacity: 0.55; font-size: 0.85em; }
.title-label {
    font-size: 1.6em;
    font-weight: 700;
    margin-bottom: 12px;
    opacity: 0.85;
}
.accent-icon { color: @accent_color; }

/* —— Control center —— */
.cc-header {
    min-height: 48px;
    padding: 6px 10px 2px 18px;
    background: transparent;
    box-shadow: none;
}

.cc-title {
    font-size: 1.08em;
    font-weight: 700;
}

.cc-title-subtitle {
    font-size: 0.72em;
    opacity: 0.42;
}

.cc-header windowcontrols button {
    min-width: 28px;
    min-height: 28px;
    border-radius: 999px;
    background: alpha(@view_fg_color, 0.045);
}

.cc-header windowcontrols button:hover {
    background: alpha(@view_fg_color, 0.1);
}

.cc-section-header {
    font-size: 0.66em;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    opacity: 0.4;
    margin-top: 18px;
    margin-bottom: 8px;
    padding-left: 2px;
}

.cc-section-header:first-child,
.cc-main > .cc-section-header:first-child {
    margin-top: 8px;
}

.cc-grid {
    margin-bottom: 4px;
}

/* Quiet surfaces let state, labels and icons carry the hierarchy. */
.cc-card {
    border-radius: 11px;
    background: linear-gradient(
        to bottom,
        mix(@window_bg_color, white, 0.04),
        mix(@window_bg_color, white, 0.022)
    );
    border: 1px solid alpha(@view_fg_color, 0.065);
    box-shadow:
        inset 0 1px 0 alpha(white, 0.025),
        0 1px 3px alpha(black, 0.1);
    transition:
        background 180ms ease-out,
        border-color 180ms ease-out,
        box-shadow 180ms ease-out,
        transform 180ms ease-out,
        color 180ms ease-out;
}

.toggle-card.flat,
.session-card.flat {
    background-image: none;
}

.toggle-card {
    padding: 13px 14px;
    min-height: 62px;
}

.toggle-card:hover,
.session-card:hover {
    background: linear-gradient(
        to bottom,
        mix(@window_bg_color, white, 0.08),
        mix(@window_bg_color, white, 0.045)
    );
    border-color: alpha(@view_fg_color, 0.14);
    box-shadow:
        inset 0 1px 0 alpha(white, 0.055),
        0 5px 16px alpha(black, 0.22);
    transform: translateY(-1.5px);
}

.toggle-card:active,
.session-card:active {
    transform: translateY(0) scale(0.985);
    box-shadow:
        inset 0 1px 0 alpha(white, 0.03),
        0 1px 4px alpha(black, 0.2);
}

/* Generic active fallback (accent). Semantic kinds override below. */
.toggle-card-active {
    background: linear-gradient(
        to bottom,
        alpha(@accent_bg_color, 0.06),
        alpha(@accent_bg_color, 0.035)
    );
    border-color: alpha(@accent_bg_color, 0.38);
    box-shadow:
        inset 0 1px 0 alpha(white, 0.05),
        0 2px 12px alpha(black, 0.14);
}

.toggle-card-active:hover {
    background: linear-gradient(
        to bottom,
        alpha(@accent_bg_color, 0.09),
        alpha(@accent_bg_color, 0.045)
    );
    border-color: alpha(@accent_bg_color, 0.5);
}

.toggle-card-active .toggle-icon {
    color: @accent_color;
    opacity: 1;
}

.toggle-card-active .toggle-title {
    opacity: 1;
}

.toggle-card-active .toggle-subtitle {
    color: @accent_color;
    opacity: 0.92;
    font-weight: 600;
}

/* —— Semantic active tints —— */
.toggle-card-wifi.toggle-card-active,
.toggle-card-bt.toggle-card-active {
    background: linear-gradient(
        to bottom,
        alpha(@accent_bg_color, 0.06),
        alpha(@accent_bg_color, 0.03)
    );
    border-color: alpha(@accent_bg_color, 0.4);
}

.toggle-card-wifi.toggle-card-active:hover,
.toggle-card-bt.toggle-card-active:hover {
    background: linear-gradient(
        to bottom,
        alpha(@accent_bg_color, 0.09),
        alpha(@accent_bg_color, 0.04)
    );
    border-color: alpha(@accent_bg_color, 0.52);
}

.toggle-card-wifi.toggle-card-active .toggle-icon,
.toggle-card-wifi.toggle-card-active .toggle-subtitle,
.toggle-card-bt.toggle-card-active .toggle-icon,
.toggle-card-bt.toggle-card-active .toggle-subtitle {
    color: @accent_color;
}

.toggle-card-nightlight.toggle-card-active {
    background: linear-gradient(
        to bottom,
        alpha(@warning_bg_color, 0.06),
        alpha(@warning_bg_color, 0.03)
    );
    border-color: alpha(@warning_bg_color, 0.4);
}

.toggle-card-nightlight.toggle-card-active:hover {
    background: linear-gradient(
        to bottom,
        alpha(@warning_bg_color, 0.09),
        alpha(@warning_bg_color, 0.04)
    );
    border-color: alpha(@warning_bg_color, 0.52);
}

.toggle-card-nightlight.toggle-card-active .toggle-icon,
.toggle-card-nightlight.toggle-card-active .toggle-subtitle {
    color: @warning_color;
}

.toggle-card-dnd.toggle-card-active {
    background: linear-gradient(
        to bottom,
        alpha(@accent_bg_color, 0.06),
        alpha(@accent_bg_color, 0.03)
    );
    border-color: alpha(@accent_bg_color, 0.4);
}

.toggle-card-dnd.toggle-card-active:hover {
    background: linear-gradient(
        to bottom,
        alpha(@accent_bg_color, 0.09),
        alpha(@accent_bg_color, 0.04)
    );
    border-color: alpha(@accent_bg_color, 0.52);
}

.toggle-card-dnd.toggle-card-active .toggle-icon,
.toggle-card-dnd.toggle-card-active .toggle-subtitle {
    color: @accent_color;
}

.toggle-icon {
    color: @view_fg_color;
    opacity: 0.76;
    background: alpha(@view_fg_color, 0.045);
    border: 1px solid alpha(@view_fg_color, 0.035);
    border-radius: 9px;
    box-shadow: none;
    padding: 7px;
    min-width: 22px;
    min-height: 22px;
    -gtk-icon-size: 22px;
}

.toggle-title {
    font-weight: 600;
    font-size: 0.94em;
}

.toggle-subtitle {
    font-size: 0.76em;
    font-weight: 450;
    opacity: 0.45;
}

.cc-sliders {
    margin-top: 10px;
    padding: 12px 14px;
}

.cc-row-icon {
    color: @accent_color;
    opacity: 0.9;
    background: alpha(@accent_bg_color, 0.09);
    border: 1px solid alpha(@accent_bg_color, 0.1);
    border-radius: 8px;
    box-shadow: none;
    padding: 6px;
    min-width: 20px;
    min-height: 20px;
    -gtk-icon-size: 20px;
}

.cc-brightness-icon {
    color: @warning_color;
    background: alpha(@warning_bg_color, 0.09);
    border-color: alpha(@warning_bg_color, 0.1);
}

.cc-scale {
    padding: 6px 0;
}

.cc-scale trough {
    min-height: 6px;
    border-radius: 999px;
    background: alpha(@view_fg_color, 0.055);
    border: none;
    box-shadow: inset 0 1px 2px alpha(black, 0.4);
}

.cc-scale highlight {
    min-height: 6px;
    border-radius: 999px;
    background: linear-gradient(
        to right,
        shade(@accent_bg_color, 1.08),
        shade(@accent_bg_color, 1.22)
    );
    border: none;
}

.cc-scale slider {
    min-width: 16px;
    min-height: 16px;
    margin: -6px 0;
    border-radius: 999px;
    background: shade(@accent_bg_color, 1.15);
    border: 2px solid shade(@accent_bg_color, 1.35);
    box-shadow:
        0 0 0 2px alpha(@accent_bg_color, 0.12),
        0 2px 6px alpha(black, 0.38),
        inset 0 1px 0 alpha(white, 0.28);
    transition: transform 180ms ease-out, box-shadow 180ms ease-out;
}

.cc-scale slider:hover {
    transform: scale(1.08);
    box-shadow:
        0 0 0 4px alpha(@accent_bg_color, 0.18),
        0 4px 12px alpha(black, 0.45),
        inset 0 1px 0 alpha(white, 0.3);
}

.cc-scale slider:active {
    transform: scale(1.14);
    box-shadow:
        0 0 0 5px alpha(@accent_bg_color, 0.22),
        0 2px 8px alpha(black, 0.45),
        inset 0 1px 0 alpha(white, 0.22);
}

.cc-scale:focus slider {
    box-shadow:
        0 0 0 4px alpha(@accent_bg_color, 0.26),
        0 3px 10px alpha(black, 0.42),
        inset 0 1px 0 alpha(white, 0.28);
}

.media-card {
    margin-top: 14px;
    padding: 16px 16px;
}

.cc-media-btn {
    border-radius: 8px;
    min-width: 34px;
    min-height: 34px;
    transition: background 180ms ease-out, transform 180ms ease-out;
}

.cc-media-btn:hover {
    background: alpha(@accent_bg_color, 0.12);
}

.cc-media-btn:active {
    transform: scale(0.94);
}

.cc-session {
    margin-top: 0;
}

.session-card {
    padding: 12px 14px;
    min-height: 58px;
}

.session-card-power {
    background: linear-gradient(
        to bottom,
        alpha(@destructive_bg_color, 0.045),
        alpha(@destructive_bg_color, 0.02)
    );
    border-color: alpha(@destructive_bg_color, 0.34);
    box-shadow:
        inset 0 1px 0 alpha(white, 0.04),
        0 2px 10px alpha(black, 0.16);
}

.session-card-power .toggle-icon,
.session-card-power .session-icon {
    color: @destructive_color;
    opacity: 0.95;
}

.session-card-power:hover {
    background: linear-gradient(
        to bottom,
        alpha(@destructive_bg_color, 0.09),
        alpha(@destructive_bg_color, 0.035)
    );
    border-color: alpha(@destructive_bg_color, 0.48);
}

/* legacy class kept for other windows */
.power-action-btn {
    min-height: 40px;
    border-radius: 10px;
    background: alpha(@view_fg_color, 0.06);
    border: 1px solid alpha(@view_fg_color, 0.08);
}
.power-action-btn:hover {
    background: alpha(@accent_bg_color, 0.12);
    border-color: alpha(@accent_bg_color, 0.3);
}

/* —— Lists (network / bluetooth / volume) —— */
.network-list,
.device-list-sub,
.history-list {
    background: alpha(@view_fg_color, 0.018);
    border: 1px solid alpha(@view_fg_color, 0.06);
    border-radius: 11px;
    padding: 4px;
}

.network-row,
.device-row {
    padding: 10px 12px;
    border-radius: 9px;
    transition: background 150ms ease;
}

.network-list row:hover,
.device-list-sub row:hover {
    background: alpha(@accent_bg_color, 0.12);
}

.network-list .bold-label,
.device-list-sub .bold-label {
    color: @accent_color;
}

/* —— App menu —— */
.app-card {
    padding: 12px 8px;
    border-radius: 11px;
    min-width: 110px;
    transition: background 150ms ease;
}
.app-card:hover {
    background: alpha(@accent_bg_color, 0.1);
}
.app-label { font-size: 0.85em; }

.costa-search {
    min-height: 42px;
    border-radius: 11px;
    background: alpha(@view_fg_color, 0.035);
    border: 1px solid alpha(@view_fg_color, 0.065);
    box-shadow: inset 0 1px 2px alpha(black, 0.18);
}

.live-result {
    background: alpha(@accent_bg_color, 0.12);
    border: 1px solid alpha(@accent_bg_color, 0.25);
    border-radius: 14px;
    padding: 12px 16px;
}
.live-result-value { font-size: 1.2em; font-weight: 700; }

/* —— Power —— */
.power-btn {
    padding: 14px;
    border-radius: 12px;
    min-width: 120px;
    min-height: 120px;
    background: linear-gradient(
        to bottom,
        mix(@window_bg_color, white, 0.04),
        mix(@window_bg_color, white, 0.022)
    );
    border: 1px solid alpha(@view_fg_color, 0.065);
    transition: background 180ms ease, border-color 180ms ease, transform 180ms ease;
}
.power-btn:hover {
    background: alpha(@accent_bg_color, 0.15);
    border-color: @accent_bg_color;
    transform: scale(1.04);
}
.power-btn:active {
    background: @accent_bg_color;
    color: @accent_fg_color;
    transform: scale(0.96);
}
.btn-label { font-size: 1.05em; font-weight: 600; }

/* —— Blinker capture tiles —— */
.capture-tile {
    padding: 18px 14px;
    border-radius: 12px;
    min-width: 120px;
    min-height: 120px;
    background: linear-gradient(
        to bottom,
        mix(@window_bg_color, white, 0.04),
        mix(@window_bg_color, white, 0.022)
    );
    border: 1px solid alpha(@view_fg_color, 0.065);
    transition: background 180ms ease, border-color 180ms ease, transform 180ms ease;
}
.capture-tile:hover {
    background: alpha(@accent_bg_color, 0.15);
    border-color: @accent_bg_color;
    transform: scale(1.04);
}
.capture-tile:active {
    background: @accent_bg_color;
    color: @accent_fg_color;
    transform: scale(0.96);
}
.capture-tile-label {
    font-size: 0.95em;
    font-weight: 700;
}
.capture-tile-shortcut {
    font-size: 0.75em;
    font-weight: 600;
    opacity: 0.45;
}

/* —— Blinker manager —— */
.screenshot-card {
    border-radius: 14px;
    background: alpha(@view_fg_color, 0.04);
    border: 1px solid alpha(@view_fg_color, 0.08);
    overflow: hidden;
    transition: border-color 160ms ease, background 160ms ease;
}
.screenshot-card:hover {
    background: alpha(@accent_bg_color, 0.1);
    border-color: alpha(@accent_bg_color, 0.35);
}
.screenshot-thumb-frame {
    background: alpha(@view_fg_color, 0.06);
    overflow: hidden;
}
.screenshot-thumb { border-radius: 0; }
.screenshot-card-footer { padding: 10px 10px 10px 12px; }
.screenshot-title { font-weight: 600; font-size: 0.85em; }
.screenshot-meta { font-size: 0.75em; opacity: 0.5; }
.screenshot-copy-btn {
    min-width: 32px;
    min-height: 32px;
    padding: 4px;
    border-radius: 10px;
}
.screenshot-copy-btn:hover {
    background: alpha(@view_fg_color, 0.08);
}

/* —— Clipper —— */
.clip-toolbar {
    background: alpha(@view_fg_color, 0.03);
    border-bottom: 1px solid alpha(@view_fg_color, 0.06);
}
.clip-search { min-width: 220px; }
.clip-sidebar {
    background: alpha(@view_fg_color, 0.02);
    border-right: 1px solid alpha(@view_fg_color, 0.06);
}
.clip-list { background: transparent; }
.clip-list row.clip-row {
    padding: 2px 8px;
    border-radius: 12px;
    transition: background 150ms ease;
}
.clip-list row.clip-row:selected {
    background: alpha(@accent_bg_color, 0.18);
}
.clip-list row.clip-row:hover:not(:selected) {
    background: alpha(@view_fg_color, 0.04);
}
.clip-type-icon { color: alpha(@view_fg_color, 0.55); }
.clip-thumb-image {
    border-radius: 6px;
    min-width: 20px;
    min-height: 20px;
}
.clip-pin-icon { color: @accent_color; opacity: 0.9; }
.clip-preview-card {
    background: alpha(@view_fg_color, 0.03);
    border: 1px solid alpha(@view_fg_color, 0.08);
    border-radius: 14px;
}
.preview-text {
    font-family: monospace;
    padding: 16px;
    background: transparent;
}
.preview-text.editable {
    background: alpha(@accent_bg_color, 0.08);
    border-radius: 10px;
}
.preview-info-pill {
    background: alpha(@window_bg_color, 0.9);
    border: 1px solid alpha(@view_fg_color, 0.08);
    border-radius: 999px;
    padding: 4px 12px;
    font-size: 0.8em;
    font-weight: 600;
    opacity: 0.85;
}
"#;
