use core::panic;
use std::{
    borrow::BorrowMut,
    cell::{Ref, RefCell, RefMut},
    ffi::c_void,
    io::{self, prelude},
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Poll, Waker},
};

use deno_core::{ExternalPointer, external, op2};
use scopeguard::{ScopeGuard, guard};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::Mutex,
};

use super::serve::HttpError;

#[derive(Debug)]
pub struct HttpParts {
    pub method: String,
    pub uri: String,
    pub version: String,
    //pub headers: HeaderMap<HeaderValue>,
}

#[derive(Debug)]
struct HttpStreamInner {
    stream: Arc<Mutex<TcpStream>>,
    request_parts: HttpParts,
    response_body: Option<String>,
    is_ready: bool,
    res_status: Option<u16>,
    response_waker: Option<Waker>,
}

#[derive(Debug)]
pub struct HttpStream(RefCell<Option<HttpStreamInner>>);

impl HttpStream {
    pub async fn new(stream: TcpStream) -> Rc<Self> {
        let stream = Arc::new(Mutex::new(stream));

        let mut guard = stream.lock().await;
        let parts = HttpStream::extract_request_parts(&mut *guard).await;

        let inner = HttpStreamInner {
            stream: stream.clone(),
            request_parts: parts,
            response_body: None,
            is_ready: false,
            res_status: None,
            response_waker: None,
        };

        Rc::new(Self(RefCell::new(Some(inner))))
    }

    fn self_ref(&self) -> Ref<'_, HttpStreamInner> {
        Ref::map(self.0.borrow(), |option| option.as_ref().unwrap())
    }

    fn self_mut(&self) -> RefMut<'_, HttpStreamInner> {
        RefMut::map(self.0.borrow_mut(), |option| option.as_mut().unwrap())
    }

    pub fn set_response_body(&self, body: String) {
        let mut inner = self.self_mut();

        inner.response_body = Some(body);
    }

    pub fn set_status(&self, status: u16) {
        let mut inner = self.self_mut();
        inner.res_status = Some(status);
    }

    pub fn complete(&self) {
        let mut inner = self.self_mut();
        inner.is_ready = true;

        if let Some(waker) = inner.response_waker.take() {
            drop(inner);
            waker.wake();
        }
    }

    async fn extract_request_parts(stream: &mut TcpStream) -> HttpParts {
        let buf = BufReader::new(stream);

        let mut lines = buf.lines();
        let Some(req_info) = lines.next_line().await.unwrap() else {
            panic!("missing http info")
        };

        let mut req_info = req_info.split_whitespace();
        let Some(method) = req_info.next() else {
            panic!("missing http method")
        };
        let Some(uri) = req_info.next() else {
            panic!("missing http uri")
        };
        let Some(version) = req_info.next() else {
            panic!("missing http version")
        };

        let parts = HttpParts {
            method: String::from(method),
            uri: String::from(uri),
            version: String::from(version),
        };

        println!("Parts: {parts:?}");

        /*
        while let Some(line) = lines.next_line().await.unwrap() {
            println!("LINE: {line}");
        }
        */

        parts
    }

    pub fn get_method(&self) -> String {
        let inner = self.self_ref();
        let parts = &inner.request_parts;

        parts.method.clone()
    }

    pub async fn read(&self) {
        // self.extract_request_parts().await;

        // let mut inner = self.self_mut();
        // let buf = BufReader::new(&mut inner.stream);

        println!("Read");

        /* printar request na tela
        let mut content = String::new();
        let mut buf = BufReader::new(&mut inner.stream);
        loop {
        let mut line = String::new();
           let Ok(bytes) = buf.read_line(&mut line).await else {
                break;
            };

            if bytes == 0 || line == "\r\n" {
                break;
            }

            content.push_str(&line);
        }
        println!("Content:\n{content}");
        */
    }

    pub async fn write(&self) {
        let inner = self.self_mut();
        let mut stream = inner.stream.lock().await;

        let response = "HTTP/1.1 200 OK\r\n\r\n";

        stream
            .write_all(response.as_bytes())
            .await
            .map_err(|_| HttpError::ServerError)
            .unwrap();

        stream
            .flush()
            .await
            .map_err(|_| HttpError::ServerError)
            .unwrap();

        /*
                loop {
                    // Wait for the socket to be writable
                    inner.stream.writable().await.unwrap();

                    // Try to write data, this may still fail with `WouldBlock`
                    // if the readiness event is a false positive.
                    match inner.stream.try_write(b"hello world") {
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
    }

    /// Resolves when response head is ready.
    fn response_ready(&self) -> impl Future<Output = ()> + '_ {
        struct HttpStreamReady<'a>(&'a HttpStream);

        impl Future for HttpStreamReady<'_> {
            type Output = ();

            fn poll(
                self: Pin<&mut Self>,
                cx: &mut core::task::Context<'_>,
            ) -> Poll<Self::Output> {
                let mut mut_self = self.0.self_mut();
                let is_ready = mut_self.is_ready;

                println!("is ready: {is_ready}");
                if is_ready {
                    return Poll::Ready(());
                }
                mut_self.response_waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }

        HttpStreamReady(self)
    }
}

#[repr(transparent)]
pub(super) struct RcHttpStream(pub Rc<HttpStream>);

// Register the [`HttpRecord`] as an external.
external!(RcHttpStream, "http stream");

/// Construct Rc<HttpRecord> from raw external pointer, consuming
/// refcount. You must make sure the external is deleted on the JS side.
macro_rules! take_external {
    ($external:expr) => {{
        let ptr = ExternalPointer::<RcHttpStream>::from_raw($external);
        let record = ptr.unsafely_take().0;
        record
    }};
}

/// Clone Rc<HttpRecord> from raw external pointer.
macro_rules! clone_external {
    ($external:expr) => {{
        let ptr = ExternalPointer::<RcHttpStream>::from_raw($external);
        ptr.unsafely_deref().0.clone()
    }};
}

#[op2]
#[string]
pub fn op_http_get_method(external: *const c_void) -> String {
    let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external) };

    http.get_method()
}

#[op2(fast)]
pub fn op_http_set_promise_complete(external: *const c_void, status: u16) {
    let http =
    // SAFETY: external is deleted before calling this op.
    unsafe {
        take_external!(external)
    };

    set_promise_complete(http, status);
}

#[op2(async)]
pub async fn op_http_read_request_body_text(external: *const c_void) {
    let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external) };

    http.read().await;
}

fn set_promise_complete(http: Rc<HttpStream>, status: u16) {
    // The Javascript code should never provide a status that is invalid here (see 23_response.js), so we
    // will quietly ignore invalid values.
    // let http = Rc::get_mut(&mut http).unwrap();
    http.set_status(status);
    http.complete();

    println!("complete: {http:?}");
}

pub(super) async fn handle_request(
    stream: TcpStream,
    tx: tokio::sync::mpsc::Sender<Rc<HttpStream>>,
) {
    let stream = HttpStream::new(stream).await;
    println!("new req: {stream:?}");

    let guarded_stream = guard(stream, |_| {});
    // Clone HttpRecord and send to JavaScript for processing.
    // Safe to unwrap as channel receiver is never closed.
    tx.send(guarded_stream.clone()).await.unwrap();

    println!("handle_request response_ready.await");
    guarded_stream.response_ready().await;

    // Defuse the guard. Must not await after this point.
    let stream = ScopeGuard::into_inner(guarded_stream);
    println!("handle_request complete");

    stream.write().await;

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
