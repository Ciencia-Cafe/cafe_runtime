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

#[derive(Debug)]
pub struct HttpStream {
    stream: TcpStream,
    response_body: Option<String>,
    is_ready: bool,
}

impl HttpStream {
    pub fn new(stream: TcpStream) -> Rc<Self> {
        Rc::new(Self {
            stream,
            response_body: None,
            is_ready: false,
        })
    }

    pub fn set_response_body(&mut self, body: String) {
        self.response_body = Some(body);
    }

    pub fn complete(&mut self) {
        self.is_ready = true;
    }

    /// Resolves when response head is ready.
    fn response_ready(&self) -> impl Future<Output = ()> + '_ {
        struct HttpStreamReady<'a>(&'a HttpStream);

        impl Future for HttpStreamReady<'_> {
            type Output = ();

            fn poll(
                self: Pin<&mut Self>,
                _cx: &mut core::task::Context<'_>,
            ) -> Poll<Self::Output> {
                let mut_self = self.0;
                if mut_self.is_ready {
                    return Poll::Ready(());
                }
                // mut_self.response_waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }

        HttpStreamReady(self)
    }
}

#[repr(transparent)]
struct RcHttpStream(Rc<HttpStream>);

// Register the [`HttpRecord`] as an external.
external!(RcHttpStream, "http stream");

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HttpError {
    #[class("BadResource")]
    #[error("Listener has been closed")]
    ServerError,
}

async fn handle_request(
    stream: TcpStream,
    tx: tokio::sync::mpsc::Sender<Rc<HttpStream>>,
) {
    let stream = HttpStream::new(stream);
    println!("new req: {stream:?}");

    let guarded_stream = guard(stream, |_| {});
    // Clone HttpRecord and send to JavaScript for processing.
    // Safe to unwrap as channel receiver is never closed.
    tx.send(guarded_stream.clone()).await.unwrap();

    println!("handle_request response_ready.await");
    guarded_stream.response_ready().await;

    /*
        loop {
            // Wait for the socket to be writable
            stream.writable().await.unwrap();

            // Try to write data, this may still fail with `WouldBlock`
            // if the readiness event is a false positive.
            match stream.try_write(b"hello world") {
                Ok(n) => {
                    println!("Write : {n:?}");
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    println!("ERROR: {e:?}");
                    continue;
                }
                Err(e) => {
                    println!("ERROR: {e:?}");
                    break;
                }
            };
        }
    */

    /*
    let response = "HTTP/1.1 200 OK\r\n\r\n";
            stream
            .write_all(b"Hello world")
            .await
            .map_err(|_| HttpError::ServerError)
            .unwrap();

        stream
            .flush()
            .await
            .map_err(|_| HttpError::ServerError)
            .unwrap();
    */
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
