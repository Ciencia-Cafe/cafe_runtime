use std::{borrow::Cow, cell::RefCell, rc::Rc};

use deno_core::{
    AsyncRefCell, CancelHandle, CancelTryFuture, OpState, RcRef, Resource,
    ResourceId, anyhow::Result, op2,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Our per-process `Connections`. We can use this to find an existent listener for
/// a given local address and clone its socket for us to listen on in our thread.
// static CONNS: std::sync::OnceLock<Arc<SocketAddr>> = std::sync::OnceLock::new();

/// A strongly-typed network listener resource for something that
/// implements `NetworkListenerTrait`.
pub struct NetworkListenerResource {
    pub listener: AsyncRefCell<TcpListener>,
    pub cancel: CancelHandle,
}

impl NetworkListenerResource {
    pub fn new(listener: TcpListener) -> Self {
        Self {
            listener: AsyncRefCell::new(listener),
            cancel: Default::default(),
        }
    }
}

impl Resource for NetworkListenerResource {
    fn name(&self) -> Cow<str> {
        "NetworkListenerResource".into()
    }

    fn close(self: Rc<Self>) {
        self.cancel.cancel();
    }
}
#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HttpError {
    #[class("BadResource")]
    #[error("Listener has been closed")]
    ServerError,
}

#[op2(fast)]
pub fn op_http_serve(state: &mut OpState) -> Result<u32, HttpError> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:8000")
        .map_err(|_| HttpError::ServerError)?;

    std_listener
        .set_nonblocking(true)
        .map_err(|_| HttpError::ServerError)?;

    let listener = TcpListener::from_std(std_listener)
        .map_err(|_| HttpError::ServerError)?;

    println!("{listener:?}");

    let rid = state
        .resource_table
        .add(NetworkListenerResource::new(listener));

    println!("rid: {rid:?}");

    Ok(rid)
}

#[op2(async)]
pub async fn op_http_accept(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
) -> Result<(), HttpError> {
    let resource = state
        .borrow()
        .resource_table
        .get::<NetworkListenerResource>(rid)
        .map_err(|_| HttpError::ServerError)?;

    let listener = RcRef::map(&resource, |r| &r.listener)
        .try_borrow_mut()
        .ok_or(HttpError::ServerError)?;

    let cancel = RcRef::map(resource, |r| &r.cancel);

    let (mut stream, _) = listener
        .accept()
        .try_or_cancel(cancel)
        .await
        .map_err(|_| HttpError::ServerError)?;

    println!("{stream:?}");

    // let mut request = String::new();

    let response = "HTTP/1.1 200 OK\r\n\r\n";

    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|_| HttpError::ServerError)?;

    stream.flush().await.map_err(|_| HttpError::ServerError)?;

    Ok(())
}
