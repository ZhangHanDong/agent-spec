use std::any::Any;

pub struct Message;

pub struct Sender;

impl Sender {
    pub fn send(&self, _message: Message) {}
}

pub struct Registry;

impl Registry {
    pub fn register(&self, _key: &str, _callback: fn()) {}
}

pub struct Router;

impl Router {
    pub fn route(&self, _path: &str, _handler: fn()) {}
}

pub fn get(handler: fn()) -> fn() {
    handler
}

pub fn worker() {}

pub fn callback_handler() {}

pub fn route_handler() {}

pub fn dispatch(
    sender: &Sender,
    registry: &Registry,
    router: &Router,
    value: &dyn Any,
) {
    tokio::spawn(worker());
    sender.send(Message);
    registry.register("save", callback_handler);
    value.downcast_ref::<Message>();
    router.route("/items", get(route_handler));
}

pub fn disconnected_target() {}

mod tokio {
    pub fn spawn<T>(_future: T) {}
}
