use std::{
    borrow::BorrowMut,
    cell::{Ref, RefCell, RefMut},
    ffi::c_void,
    io,
    pin::Pin,
    rc::Rc,
    task::{Poll, Waker},
};

use deno_core::{ExternalPointer, external, op2};
use scopeguard::{ScopeGuard, guard};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use super::serve::HttpError;

#[derive(Debug)]
struct HttpStreamInner {
    stream: TcpStream,
    response_body: Option<String>,
    is_ready: bool,
    res_status: Option<u16>,
    response_waker: Option<Waker>,
}

#[derive(Debug)]
pub struct HttpStream(RefCell<Option<HttpStreamInner>>);

impl HttpStream {
    pub fn new(stream: TcpStream) -> Rc<Self> {
        Rc::new(Self(RefCell::new(Some(HttpStreamInner {
            stream,
            response_body: None,
            is_ready: false,
            res_status: None,
            response_waker: None,
        }))))
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

    pub async fn write(&self) {
        let mut inner = self.self_mut();

        let response = "HTTP/1.1 200 OK\r\n\r\n";
        inner
            .stream
            .write_all(response.as_bytes())
            .await
            .map_err(|_| HttpError::ServerError)
            .unwrap();

        inner
            .stream
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

#[op2(fast)]
pub fn op_http_set_promise_complete(external: *const c_void, status: u16) {
    let http =
    // SAFETY: external is deleted before calling this op.
    unsafe {
        let ptr = ExternalPointer::<RcHttpStream>::from_raw(external);
        let record = ptr.unsafely_take().0;
        record
    };

    set_promise_complete(http, status);
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
    let stream = HttpStream::new(stream);
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
