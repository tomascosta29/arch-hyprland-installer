//! Run blocking backend work off the GTK thread and deliver on the main loop.

use gtk4::glib;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

pub fn spawn_result<T, F, S, E>(worker: F, on_success: S, on_error: E)
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, costa_core::Error> + Send + 'static,
    S: FnOnce(T) + 'static,
    E: FnOnce(costa_core::Error) + 'static,
{
    let (tx, rx) = mpsc::sync_channel::<Result<T, costa_core::Error>>(1);
    std::thread::Builder::new()
        .name("costa-job".into())
        .spawn(move || {
            let _ = tx.send(worker());
        })
        .expect("spawn costa job thread");

    let on_success = RefCell::new(Some(on_success));
    let on_error = RefCell::new(Some(on_error));
    glib::timeout_add_local(Duration::from_millis(16), move || match rx.try_recv() {
        Ok(Ok(value)) => {
            if let Some(cb) = on_success.borrow_mut().take() {
                cb(value);
            }
            glib::ControlFlow::Break
        }
        Ok(Err(err)) => {
            if let Some(cb) = on_error.borrow_mut().take() {
                cb(err);
            }
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });
}

/// Debounce rapid GTK signals into the latest call.
pub struct Debouncer {
    delay: Duration,
    callback: Rc<RefCell<Box<dyn FnMut(f64)>>>,
    source: Rc<RefCell<Option<glib::SourceId>>>,
    pending: Rc<RefCell<Option<f64>>>,
}

impl Debouncer {
    pub fn new(delay_ms: u64, callback: impl FnMut(f64) + 'static) -> Self {
        Self {
            delay: Duration::from_millis(delay_ms),
            callback: Rc::new(RefCell::new(Box::new(callback))),
            source: Rc::new(RefCell::new(None)),
            pending: Rc::new(RefCell::new(None)),
        }
    }

    pub fn schedule(&self, value: f64) {
        *self.pending.borrow_mut() = Some(value);
        if self.source.borrow().is_some() {
            return;
        }
        let callback = self.callback.clone();
        let pending = self.pending.clone();
        let source = self.source.clone();
        let id = glib::timeout_add_local(self.delay, move || {
            *source.borrow_mut() = None;
            if let Some(value) = pending.borrow_mut().take() {
                (callback.borrow_mut())(value);
            }
            glib::ControlFlow::Break
        });
        *self.source.borrow_mut() = Some(id);
    }

    pub fn cancel(&self) {
        if let Some(id) = self.source.borrow_mut().take() {
            id.remove();
        }
        *self.pending.borrow_mut() = None;
    }
}
