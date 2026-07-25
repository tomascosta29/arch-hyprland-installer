use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use adw::prelude::*;
use costa_core::backends::{PowerAction, PowerBackend};
use gtk4::{gdk, glib, CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tracing::error;

pub struct PowerWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
}

impl PowerWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Power Menu")
            .default_width(480)
            .default_height(420)
            .resizable(false)
            .build();

        let backend = PowerBackend::new();
        let pending = Rc::new(Cell::new(None::<PowerAction>));
        let confirm_source = Rc::new(Cell::new(None::<glib::SourceId>));
        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        let labels: Rc<RefCell<Vec<(PowerAction, gtk4::Label, gtk4::Button)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 24);
        main_box.set_halign(gtk4::Align::Center);
        main_box.set_valign(gtk4::Align::Center);
        main_box.set_margin_top(32);
        main_box.set_margin_bottom(32);
        main_box.set_margin_start(32);
        main_box.set_margin_end(32);
        window.set_content(Some(&main_box));

        let title = gtk4::Label::new(Some("Power Menu"));
        title.add_css_class("title-label");
        main_box.append(&title);

        let flow = gtk4::FlowBox::new();
        flow.set_valign(gtk4::Align::Center);
        flow.set_halign(gtk4::Align::Center);
        flow.set_selection_mode(gtk4::SelectionMode::None);
        flow.set_max_children_per_line(3);
        flow.set_min_children_per_line(3);
        flow.set_column_spacing(20);
        flow.set_row_spacing(20);
        main_box.append(&flow);

        let hint = gtk4::Label::new(Some("Press Esc to cancel"));
        hint.add_css_class("dim-label");
        main_box.append(&hint);

        for action in PowerAction::all() {
            let (button, label) = make_button(*action);
            labels
                .borrow_mut()
                .push((*action, label.clone(), button.clone()));

            let pending = pending.clone();
            let confirm_source = confirm_source.clone();
            let labels = labels.clone();
            let window_weak = window.downgrade();
            let backend = backend.clone();

            button.connect_clicked(move |_btn| {
                if action.requires_confirm() && pending.get() != Some(*action) {
                    reset_confirmation(&pending, &confirm_source, &labels);
                    pending.set(Some(*action));
                    if let Some((_, label, button)) =
                        labels.borrow().iter().find(|(a, _, _)| *a == *action)
                    {
                        label.set_label(&format!("Confirm {}", action.label()));
                        button.add_css_class("destructive-action");
                    }
                    let pending_c = pending.clone();
                    let confirm_source_c = confirm_source.clone();
                    let labels_c = labels.clone();
                    let id = glib::timeout_add_seconds_local(4, move || {
                        confirm_source_c.set(None);
                        reset_confirmation(&pending_c, &confirm_source_c, &labels_c);
                        glib::ControlFlow::Break
                    });
                    confirm_source.set(Some(id));
                    return;
                }

                reset_confirmation(&pending, &confirm_source, &labels);
                if let Some(win) = window_weak.upgrade() {
                    win.set_visible(false);
                }
                if let Err(err) = backend.execute(*action) {
                    error!(?action, %err, "power action failed");
                }
            });

            flow.append(&button);
        }

        install_popup_dismiss(&window, focus_guard.clone());
        load_css();

        Self {
            window,
            focus_guard,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
    }
}

fn make_button(action: PowerAction) -> (gtk4::Button, gtk4::Label) {
    let button = gtk4::Button::new();
    button.add_css_class("power-btn");

    let box_ = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    box_.set_halign(gtk4::Align::Center);
    box_.set_valign(gtk4::Align::Center);

    let icon = gtk4::Image::from_icon_name(action.icon());
    icon.set_pixel_size(48);

    let label = gtk4::Label::new(Some(action.label()));
    label.add_css_class("btn-label");

    box_.append(&icon);
    box_.append(&label);
    button.set_child(Some(&box_));
    (button, label)
}

fn reset_confirmation(
    pending: &Cell<Option<PowerAction>>,
    confirm_source: &Cell<Option<glib::SourceId>>,
    labels: &RefCell<Vec<(PowerAction, gtk4::Label, gtk4::Button)>>,
) {
    pending.set(None);
    if let Some(id) = confirm_source.take() {
        id.remove();
    }
    for (action, label, button) in labels.borrow().iter() {
        label.set_label(action.label());
        button.remove_css_class("destructive-action");
    }
}

fn load_css() {
    static LOADED: std::sync::Once = std::sync::Once::new();
    LOADED.call_once(|| {
        let provider = CssProvider::new();
        provider.load_from_string(
            r#"
            window { background: alpha(@window_bg_color, 0.95); }
            .title-label { font-size: 1.8em; font-weight: 800; margin-bottom: 12px; opacity: 0.8; }
            .power-btn {
                padding: 16px;
                border-radius: 16px;
                min-width: 120px;
                min-height: 120px;
                background: alpha(@view_fg_color, 0.05);
                border: 1px solid alpha(@view_fg_color, 0.08);
                transition: all 200ms cubic-bezier(0.25, 0.46, 0.45, 0.94);
            }
            .power-btn:hover {
                background: alpha(@accent_bg_color, 0.15);
                border-color: @accent_bg_color;
                transform: scale(1.05);
            }
            .power-btn:active {
                background: @accent_bg_color;
                color: @accent_fg_color;
                transform: scale(0.95);
            }
            .btn-label { font-size: 1.1em; font-weight: bold; }
            .dim-label { opacity: 0.5; font-size: 0.9em; margin-top: 12px; }
            "#,
        );
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
