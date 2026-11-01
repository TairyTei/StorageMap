use futures::channel::oneshot;
use slint::{CloseRequestResponse, ComponentHandle};
use std::{cell::RefCell, rc::Rc};

pub struct Reply<T>(Rc<RefCell<Option<oneshot::Sender<Option<T>>>>>);

impl<T> Clone for Reply<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Reply<T> {
    pub fn send(&self, v: Option<T>) {
        if let Some(tx) = self.0.borrow_mut().take() {
            let _ = tx.send(v);
        }
    }
}

pub async fn run_dialog<D, T>(dlg: D, wire: impl FnOnce(&D, Reply<T>)) -> Option<T>
where
    D: ComponentHandle,
    T: 'static,
{
    let (tx, rx) = oneshot::channel();
    let reply = Reply(Rc::new(RefCell::new(Some(tx))));

    wire(&dlg, reply.clone());

    dlg.window().on_close_requested({
        let reply = reply.clone();
        move || {
            reply.send(None);
            CloseRequestResponse::HideWindow
        }
    });

    dlg.show().ok()?;
    let result = rx.await.ok().flatten();
    let _ = dlg.hide();
    result
}
