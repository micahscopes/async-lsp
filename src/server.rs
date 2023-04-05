use std::future::{ready, Future, Ready};
use std::ops::ControlFlow;
use std::task::{Context, Poll};

use either::Either;
use lsp_types::notification::{self, Notification};
use lsp_types::request::{self, Request};
use tower_layer::Layer;
use tower_service::Service;

use crate::{
    AnyEvent, AnyNotification, AnyRequest, Error, ErrorCode, JsonValue, LspService, ResponseError,
    Result,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Uninitialized,
    Initializing,
    Ready,
    ShuttingDown,
}

#[derive(Debug, Default)]
pub struct Lifecycle<S> {
    service: S,
    state: State,
}

impl<S> Lifecycle<S> {
    pub fn new(service: S) -> Self {
        Self {
            service,
            state: State::Uninitialized,
        }
    }
}

impl<S: LspService> Service<AnyRequest> for Lifecycle<S> {
    type Response = JsonValue;
    type Error = ResponseError;
    type Future = Either<S::Future, Ready<<S::Future as Future>::Output>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: AnyRequest) -> Self::Future {
        match (self.state, &*req.method) {
            (State::Uninitialized, request::Initialize::METHOD) => {
                self.state = State::Initializing;
                Either::Left(self.service.call(req))
            }
            (State::Uninitialized | State::Initializing, _) => {
                Either::Right(ready(Err(ResponseError {
                    code: ErrorCode::SERVER_NOT_INITIALIZED,
                    message: "Server is not initialized yet".into(),
                    data: None,
                })))
            }
            (_, request::Initialize::METHOD) => Either::Right(ready(Err(ResponseError {
                code: ErrorCode::INVALID_REQUEST,
                message: "Server is already initialized".into(),
                data: None,
            }))),
            (State::Ready, _) => {
                if req.method == request::Shutdown::METHOD {
                    self.state = State::ShuttingDown;
                }
                Either::Left(self.service.call(req))
            }
            (State::ShuttingDown, _) => Either::Right(ready(Err(ResponseError {
                code: ErrorCode::INVALID_REQUEST,
                message: "Server is shutting down".into(),
                data: None,
            }))),
        }
    }
}

impl<S: LspService> LspService for Lifecycle<S> {
    fn notify(&mut self, notif: AnyNotification) -> ControlFlow<Result<()>> {
        match &*notif.method {
            notification::Exit::METHOD => ControlFlow::Break(Ok(())),
            notification::Initialized::METHOD => {
                if self.state != State::Initializing {
                    return ControlFlow::Break(Err(Error::Protocol(format!(
                        "Unexpected initialized notification on state {:?}",
                        self.state
                    ))));
                }
                self.state = State::Ready;
                ControlFlow::Continue(())
            }
            _ => self.service.notify(notif),
        }
    }

    fn emit(&mut self, event: AnyEvent) -> ControlFlow<Result<()>> {
        self.service.emit(event)
    }
}

pub struct LifecycleLayer;

impl<S> Layer<S> for LifecycleLayer {
    type Service = Lifecycle<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Lifecycle::new(inner)
    }
}
