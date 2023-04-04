use std::ops::ControlFlow;
use std::time::Duration;

use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::monitor::ClientProcessMonitorLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router::Router;
use async_lsp::server::LifecycleLayer;
use async_lsp::Client;
use lsp_types::{
    notification, request, Hover, HoverContents, HoverProviderCapability, InitializeResult,
    MarkedString, MessageType, OneOf, ServerCapabilities, ShowMessageParams,
};
use tower::ServiceBuilder;

struct ServerState {
    client: Client,
    counter: i32,
}

struct TickEvent;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let server = async_lsp::Server::new(1, |client| {
        tokio::spawn({
            let client = client.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    if client.emit(TickEvent).await.is_err() {
                        break;
                    };
                }
            }
        });

        let mut router = Router::new(ServerState {
            client: client.clone(),
            counter: 0,
        });
        router
            .request::<request::Initialize, _>(|_, params| async move {
                eprintln!("Initialize with {params:?}");
                Ok(InitializeResult {
                    capabilities: ServerCapabilities {
                        hover_provider: Some(HoverProviderCapability::Simple(true)),
                        definition_provider: Some(OneOf::Left(true)),
                        ..ServerCapabilities::default()
                    },
                    server_info: None,
                })
            })
            .request::<request::HoverRequest, _>(|st, _| {
                let client = st.client.clone();
                let counter = st.counter;
                async move {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    client
                        .notify::<notification::ShowMessage>(ShowMessageParams {
                            typ: MessageType::INFO,
                            message: "Hello LSP".into(),
                        })
                        .await
                        .unwrap();
                    Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(format!(
                            "I am a hover text {counter}!"
                        ))),
                        range: None,
                    }))
                }
            })
            .request::<request::GotoDefinition, _>(|_, _| async move {
                unimplemented!("Not yet implemented!")
            })
            .notification::<notification::DidChangeConfiguration>(|_, _| ControlFlow::Continue(()))
            .notification::<notification::DidOpenTextDocument>(|_, _| ControlFlow::Continue(()))
            .notification::<notification::DidChangeTextDocument>(|_, _| ControlFlow::Continue(()))
            .notification::<notification::DidCloseTextDocument>(|_, _| ControlFlow::Continue(()))
            .event::<TickEvent>(|st, _| {
                st.counter += 1;
                ControlFlow::Continue(())
            });

        ServiceBuilder::new()
            .layer(LifecycleLayer)
            .layer(CatchUnwindLayer::new())
            .layer(ConcurrencyLayer::new(4))
            .layer(ClientProcessMonitorLayer::new(client))
            .service(router)
    });
    server.run().await.unwrap();
}
