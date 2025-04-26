use std::{
    borrow::Cow, cell::RefCell, io, net::SocketAddr, pin::Pin, ptr::null,
    rc::Rc, sync::Arc, task::Poll,
};

use deno_core::{
    AsyncRefCell, CancelTryFuture, ExternalPointer, OpState, RcRef, Resource,
    ResourceId, anyhow::Result, external, op2,
};
use scopeguard::guard;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    spawn,
    task::JoinHandle,
};

use super::http_stream::{HttpStream, RcHttpStream, handle_request};

/// Our per-process `Connections`. We can use this to find an existent listener for
/// a given local address and clone its socket for us to listen on in our thread.
// static CONNS: std::sync::OnceLock<Arc<SocketAddr>> = std::sync::OnceLock::new();

/// A strongly-typed network listener resource for something that
/// implements `NetworkListenerTrait`.
/*
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

*/

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HttpError {
    #[class("BadResource")]
    #[error("Listener has been closed")]
    ServerError,
}

struct HttpJoinHandle {
    join_handle:
        AsyncRefCell<Option<deno_unsync::JoinHandle<Result<(), HttpError>>>>,
    rx: AsyncRefCell<tokio::sync::mpsc::Receiver<Rc<HttpStream>>>,
}
impl HttpJoinHandle {
    fn new(rx: tokio::sync::mpsc::Receiver<Rc<HttpStream>>) -> Self {
        Self {
            join_handle: AsyncRefCell::new(None),
            rx: AsyncRefCell::new(rx),
        }
    }
}
impl Resource for HttpJoinHandle {
    fn name(&self) -> Cow<str> {
        "http".into()
    }

    fn close(self: Rc<Self>) {}
}

#[op2(fast)]
pub fn op_http_serve(state: Rc<RefCell<OpState>>) -> Result<u32, HttpError> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:8000")
        .map_err(|_| HttpError::ServerError)?;

    std_listener
        .set_nonblocking(true)
        .map_err(|_| HttpError::ServerError)?;

    let listener = TcpListener::from_std(std_listener)
        .map_err(|_| HttpError::ServerError)?;

    let listener = Arc::new(listener);
    println!("{listener:?}");

    let (tx, rx) = tokio::sync::mpsc::channel(10);
    let resource: Rc<HttpJoinHandle> = Rc::new(HttpJoinHandle::new(rx));

    let handle = deno_unsync::spawn(async move {
        println!("waiting for reqs");
        let conn = Arc::clone(&listener);

        loop {
            let (stream, _) = conn
                .accept()
                .await
                .map_err(|_| HttpError::ServerError)
                .unwrap();

            let _next = handle_request(stream, tx.clone()).await;
        }
    });

    // Set the handle after we start the future
    *RcRef::map(&resource, |this| &this.join_handle)
        .try_borrow_mut()
        .unwrap() = Some(handle);

    let rid = state.borrow_mut().resource_table.add_rc(resource);

    Ok(rid)
}

#[op2(async)]
pub async fn op_http_wait(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
) -> Result<*const std::ffi::c_void, HttpError> {
    // We will get the join handle initially, as we might be consuming requests still
    let join_handle = state
        .borrow_mut()
        .resource_table
        .get::<HttpJoinHandle>(rid)
        .map_err(|_| HttpError::ServerError)?;

    // receive incomming requests here
    let next = async {
        let mut recv =
            RcRef::map(&join_handle, |this| &this.rx).borrow_mut().await;

        recv.recv().await
    }
    .await;

    // Send incomming request to Js land
    if let Some(record) = next {
        let ptr = ExternalPointer::new(RcHttpStream(record));
        return Ok(ptr.into_raw());
    }

    let _res = RcRef::map(join_handle, |this| &this.join_handle)
        .borrow_mut()
        .await
        .take()
        .unwrap()
        .await
        .unwrap();

    Ok(null())
}

/*
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
*/
