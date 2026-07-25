use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Temporary stand-in while a target is still being ported from Python.
pub struct PlaceholderWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
}

impl PlaceholderWindow {
    pub fn new(app: &adw::Application, title: &str, detail: &str) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(title)
            .default_width(420)
            .default_height(220)
            .resizable(false)
            .build();

        let box_ = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        box_.set_halign(gtk4::Align::Center);
        box_.set_valign(gtk4::Align::Center);
        box_.set_margin_top(24);
        box_.set_margin_bottom(24);
        box_.set_margin_start(24);
        box_.set_margin_end(24);

        let heading = gtk4::Label::new(Some(title));
        heading.add_css_class("title-2");
        box_.append(&heading);

        let body = gtk4::Label::new(Some(detail));
        body.set_wrap(true);
        body.add_css_class("dim-label");
        box_.append(&body);

        let hint = gtk4::Label::new(Some("Press Esc to close"));
        hint.add_css_class("dim-label");
        box_.append(&hint);

        window.set_content(Some(&box_));

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        install_popup_dismiss(&window, focus_guard.clone());

        Self {
            window,
            focus_guard,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
    }
}
