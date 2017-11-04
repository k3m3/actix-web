use std::rc::Rc;

use actix::dev::*;
use bytes::Bytes;
use futures::{Future, Poll, Async};
use tokio_io::{AsyncRead, AsyncWrite};

use h1;
use h2;
use task::Task;
use payload::Payload;
use httprequest::HttpRequest;

/// Low level http request handler
pub trait HttpHandler: 'static {
    /// Http handler prefix
    fn prefix(&self) -> &str;
    /// Handle request
    fn handle(&self, req: &mut HttpRequest, payload: Payload) -> Task;
}

enum HttpProtocol<T, A, H>
    where T: AsyncRead + AsyncWrite + 'static, A: 'static, H: 'static
{
    H1(h1::Http1<T, A, H>),
    H2(h2::Http2<T, A, H>),
}

pub struct HttpChannel<T, A, H>
    where T: AsyncRead + AsyncWrite + 'static, A: 'static, H: 'static
{
    proto: Option<HttpProtocol<T, A, H>>,
}

impl<T, A, H> HttpChannel<T, A, H>
    where T: AsyncRead + AsyncWrite + 'static, A: 'static, H: HttpHandler + 'static
{
    pub fn new(stream: T, addr: A, router: Rc<Vec<H>>, http2: bool) -> HttpChannel<T, A, H> {
        if http2 {
            HttpChannel {
                proto: Some(HttpProtocol::H2(
                    h2::Http2::new(stream, addr, router, Bytes::new()))) }
        } else {
            HttpChannel {
                proto: Some(HttpProtocol::H1(
                    h1::Http1::new(stream, addr, router))) }
        }
    }
}

/*impl<T: 'static, A: 'static, H: 'static> Drop for HttpChannel<T, A, H> {
    fn drop(&mut self) {
        println!("Drop http channel");
    }
}*/

impl<T, A, H> Actor for HttpChannel<T, A, H>
    where T: AsyncRead + AsyncWrite + 'static, A: 'static, H: HttpHandler + 'static
{
    type Context = Context<Self>;
}

impl<T, A, H> Future for HttpChannel<T, A, H>
    where T: AsyncRead + AsyncWrite + 'static, A: 'static, H: HttpHandler + 'static
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.proto {
            Some(HttpProtocol::H1(ref mut h1)) => {
                match h1.poll() {
                    Ok(Async::Ready(h1::Http1Result::Done)) =>
                        return Ok(Async::Ready(())),
                    Ok(Async::Ready(h1::Http1Result::Upgrade)) => (),
                    Ok(Async::NotReady) =>
                        return Ok(Async::NotReady),
                    Err(_) =>
                        return Err(()),
                }
            }
            Some(HttpProtocol::H2(ref mut h2)) =>
                return h2.poll(),
            None => unreachable!(),
        }

        // upgrade to h2
        let proto = self.proto.take().unwrap();
        match proto {
            HttpProtocol::H1(h1) => {
                let (stream, addr, router, buf) = h1.into_inner();
                self.proto = Some(HttpProtocol::H2(h2::Http2::new(stream, addr, router, buf)));
                self.poll()
            }
            _ => unreachable!()
        }
    }
}
