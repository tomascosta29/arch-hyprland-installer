//! BlueZ Agent1 registration with Adwaita confirmation dialogs.

use adw::prelude::*;
use gtk4::glib;
use gtk4::prelude::IsA;
use std::cell::RefCell;
use std::rc::Rc;

const AGENT_PATH: &str = "/org/fcosta/CostaUtils/BluetoothAgent";
const AGENT_XML: &str = r#"
<node>
  <interface name="org.bluez.Agent1">
    <method name="Release"/>
    <method name="RequestPinCode">
      <arg name="device" direction="in" type="o"/>
      <arg name="pincode" direction="out" type="s"/>
    </method>
    <method name="DisplayPinCode">
      <arg name="device" direction="in" type="o"/>
      <arg name="pincode" direction="in" type="s"/>
    </method>
    <method name="RequestPasskey">
      <arg name="device" direction="in" type="o"/>
      <arg name="passkey" direction="out" type="u"/>
    </method>
    <method name="DisplayPasskey">
      <arg name="device" direction="in" type="o"/>
      <arg name="passkey" direction="in" type="u"/>
      <arg name="entered" direction="in" type="q"/>
    </method>
    <method name="RequestConfirmation">
      <arg name="device" direction="in" type="o"/>
      <arg name="passkey" direction="in" type="u"/>
    </method>
    <method name="RequestAuthorization">
      <arg name="device" direction="in" type="o"/>
    </method>
    <method name="AuthorizeService">
      <arg name="device" direction="in" type="o"/>
      <arg name="uuid" direction="in" type="s"/>
    </method>
    <method name="Cancel"/>
  </interface>
</node>
"#;

pub struct PairingAgent {
    registration: Option<gio::RegistrationId>,
    connection: gio::DBusConnection,
    dialog: Rc<RefCell<Option<adw::MessageDialog>>>,
}

impl PairingAgent {
    pub fn register(
        parent: &impl IsA<gtk4::Window>,
        toast: &adw::ToastOverlay,
    ) -> Option<Self> {
        let connection = gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE).ok()?;
        let node = gio::DBusNodeInfo::for_xml(AGENT_XML).ok()?;
        let iface = node.interfaces().first()?.clone();
        let dialog = Rc::new(RefCell::new(None::<adw::MessageDialog>));
        let parent = parent.clone().upcast::<gtk4::Window>();
        let toast_cb = toast.clone();
        let toast_err = toast.clone();
        let dialog_slot = dialog.clone();

        let registration = connection
            .register_object(AGENT_PATH, &iface)
            .method_call(move |_conn, _sender, _path, _iface, method, parameters, invocation| {
                handle_method(
                    &parent,
                    &toast_cb,
                    &dialog_slot,
                    method,
                    parameters,
                    invocation,
                );
            })
            .build()
            .ok()?;

        if let Err(err) = connection.call_sync(
            Some("org.bluez"),
            "/org/bluez",
            "org.bluez.AgentManager1",
            "RegisterAgent",
            Some(&(AGENT_PATH, "KeyboardDisplay").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            5000,
            gio::Cancellable::NONE,
        ) {
            toast_register_error(toast_err, &err);
            let _ = connection.unregister_object(registration);
            return None;
        }
        let _ = connection.call_sync(
            Some("org.bluez"),
            "/org/bluez",
            "org.bluez.AgentManager1",
            "RequestDefaultAgent",
            Some(&(AGENT_PATH,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            5000,
            gio::Cancellable::NONE,
        );

        Some(Self {
            registration: Some(registration),
            connection,
            dialog,
        })
    }
}

fn toast_register_error(toast: adw::ToastOverlay, err: &glib::Error) {
    toast.add_toast(adw::Toast::new(&format!(
        "Bluetooth pairing agent unavailable: {err}"
    )));
}

impl Drop for PairingAgent {
    fn drop(&mut self) {
        let _ = self.connection.call_sync(
            Some("org.bluez"),
            "/org/bluez",
            "org.bluez.AgentManager1",
            "UnregisterAgent",
            Some(&(AGENT_PATH,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            2000,
            gio::Cancellable::NONE,
        );
        if let Some(id) = self.registration.take() {
            let _ = self.connection.unregister_object(id);
        }
        if let Some(dialog) = self.dialog.borrow_mut().take() {
            dialog.close();
        }
    }
}

fn handle_method(
    parent: &gtk4::Window,
    toast: &adw::ToastOverlay,
    dialog_slot: &Rc<RefCell<Option<adw::MessageDialog>>>,
    method: &str,
    parameters: glib::Variant,
    invocation: gio::DBusMethodInvocation,
) {
    match method {
        "Release" => invocation.return_value(None),
        "RequestPinCode" => ask(
            parent,
            dialog_slot,
            "Bluetooth PIN",
            "Enter the device PIN",
            Some(EntryKind::Pin),
            invocation,
        ),
        "RequestPasskey" => ask(
            parent,
            dialog_slot,
            "Bluetooth passkey",
            "Enter the six-digit device passkey",
            Some(EntryKind::Passkey),
            invocation,
        ),
        "RequestConfirmation" => {
            let passkey = parameters.child_value(1).get::<u32>().unwrap_or(0);
            ask(
                parent,
                dialog_slot,
                "Confirm Bluetooth pairing",
                &format!("Does the device show {passkey:06}?"),
                None,
                invocation,
            );
        }
        "RequestAuthorization" | "AuthorizeService" => ask(
            parent,
            dialog_slot,
            "Authorize Bluetooth device",
            "Allow this device to connect?",
            None,
            invocation,
        ),
        "DisplayPinCode" => {
            let pin = parameters.child_value(1).str().unwrap_or("").to_string();
            toast.add_toast(adw::Toast::new(&format!(
                "Enter PIN {pin} on the Bluetooth device"
            )));
            invocation.return_value(None);
        }
        "DisplayPasskey" => {
            let passkey = parameters.child_value(1).get::<u32>().unwrap_or(0);
            toast.add_toast(adw::Toast::new(&format!(
                "Enter passkey {passkey:06} on the Bluetooth device"
            )));
            invocation.return_value(None);
        }
        "Cancel" => {
            if let Some(dialog) = dialog_slot.borrow_mut().take() {
                dialog.close();
            }
            invocation.return_value(None);
        }
        other => invocation.return_dbus_error(
            "org.bluez.Error.NotSupported",
            &format!("Unsupported agent method: {other}"),
        ),
    }
}

#[derive(Clone, Copy)]
enum EntryKind {
    Pin,
    Passkey,
}

fn ask(
    parent: &gtk4::Window,
    dialog_slot: &Rc<RefCell<Option<adw::MessageDialog>>>,
    title: &str,
    body: &str,
    entry_kind: Option<EntryKind>,
    invocation: gio::DBusMethodInvocation,
) {
    let dialog = adw::MessageDialog::new(Some(parent), Some(title), Some(body));
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("accept", "Confirm");
    dialog.set_response_appearance("accept", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("accept"));
    dialog.set_close_response("cancel");

    let entry = entry_kind.map(|kind| {
        let entry = gtk4::Entry::new();
        match kind {
            EntryKind::Pin => {
                entry.set_input_purpose(gtk4::InputPurpose::Pin);
                entry.set_max_length(16);
            }
            EntryKind::Passkey => {
                entry.set_input_purpose(gtk4::InputPurpose::Digits);
                entry.set_max_length(6);
            }
        }
        dialog.set_extra_child(Some(&entry));
        entry
    });

    let dialog_slot = dialog_slot.clone();
    let dialog_for_store = dialog_slot.clone();
    let invocation = RefCell::new(Some(invocation));
    dialog.connect_response(None, move |_dialog, response| {
        *dialog_slot.borrow_mut() = None;
        let Some(invocation) = invocation.borrow_mut().take() else {
            return;
        };
        if response != "accept" {
            invocation.return_dbus_error("org.bluez.Error.Rejected", "Pairing rejected");
            return;
        }
        match entry_kind {
            Some(EntryKind::Pin) => {
                let value = entry
                    .as_ref()
                    .map(|e| e.text().to_string())
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if value.is_empty() {
                    invocation.return_dbus_error("org.bluez.Error.Rejected", "Pairing rejected");
                } else {
                    invocation.return_value(Some(&(value,).to_variant()));
                }
            }
            Some(EntryKind::Passkey) => {
                let value = entry
                    .as_ref()
                    .map(|e| e.text().to_string())
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let parsed = value.parse::<u32>().ok();
                if !value.chars().all(|c| c.is_ascii_digit())
                    || parsed.is_none_or(|n| n > 999_999)
                {
                    invocation.return_dbus_error("org.bluez.Error.Rejected", "Pairing rejected");
                } else {
                    invocation.return_value(Some(&(parsed.unwrap_or(0),).to_variant()));
                }
            }
            None => invocation.return_value(None),
        }
    });

    *dialog_for_store.borrow_mut() = Some(dialog.clone());
    dialog.present();
}
