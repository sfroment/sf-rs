use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(crate) type JsCallback = js_sys::Function;

type CallbackMap = RefCell<HashMap<usize, JsCallback>>;

#[derive(Debug)]
pub(crate) struct JsCallbackManager {
    callbacks: CallbackMap,
    next_id: AtomicUsize,
}

impl JsCallbackManager {
    pub(crate) fn new() -> Self {
        Self {
            callbacks: RefCell::new(HashMap::new()),
            next_id: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub(crate) fn add(&self, callback: JsCallback) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.callbacks.borrow_mut().insert(id, callback);
        id
    }

    #[inline]
    pub(crate) fn remove(&self, id: usize) {
        self.callbacks.borrow_mut().remove(&id);
    }

    #[inline]
    pub(crate) fn borrow_callbacks(&self) -> Ref<'_, HashMap<usize, JsCallback>> {
        self.callbacks.borrow()
    }
}

impl Default for JsCallbackManager {
    fn default() -> Self {
        Self::new()
    }
}
