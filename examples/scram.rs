use std::sync::Arc;

use async_trait::async_trait;

use tokio::net::TcpListener;

use pgwire::api::auth::scram::{gen_salted_password, AuthDB, MakeSASLScramAuthStartupHandler};
use pgwire::api::auth::NoopServerParameterProvider;
use pgwire::api::portal::Portal;
use pgwire::api::query::{ExtendedQueryHandler, SimpleQueryHandler};
use pgwire::api::results::{Response, Tag};
use pgwire::api::{ClientInfo, StatelessMakeHandler};
use pgwire::error::PgWireResult;
use pgwire::tokio::process_socket;

pub struct DummyProcessor;

#[async_trait]
impl SimpleQueryHandler for DummyProcessor {
    async fn do_query<C>(&self, _client: &C, _query: &str) -> PgWireResult<Vec<Response>>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        Ok(vec![Response::Execution(Tag::new_for_execution(
            "OK",
            Some(1),
        ))])
    }
}

#[async_trait]
impl ExtendedQueryHandler for DummyProcessor {
    async fn do_query<C>(
        &self,
        _client: &mut C,
        _portal: &Portal,
        _max_rows: usize,
    ) -> PgWireResult<Response>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        Ok(Response::Execution(Tag::new_for_execution("OK", Some(1))))
    }
}

struct DummyAuthDB;

#[async_trait]
impl AuthDB for DummyAuthDB {
    async fn get_salted_password(
        &self,
        _username: &str,
        salt: &[u8],
        iterations: usize,
    ) -> PgWireResult<Vec<u8>> {
        let password = "pencil";
        Ok(gen_salted_password(password, salt, iterations))
    }
}

#[tokio::main]
pub async fn main() {
    let processor = Arc::new(StatelessMakeHandler::new(Arc::new(DummyProcessor)));
    let authenticator = Arc::new(MakeSASLScramAuthStartupHandler::new(
        Arc::new(DummyAuthDB),
        Arc::new(NoopServerParameterProvider),
    ));

    let server_addr = "127.0.0.1:5432";
    let listener = TcpListener::bind(server_addr).await.unwrap();
    println!("Listening to {}", server_addr);
    loop {
        let incoming_socket = listener.accept().await.unwrap();
        let authenticator_ref = authenticator.clone();
        let processor_ref = processor.clone();
        tokio::spawn(async move {
            process_socket(
                incoming_socket.0,
                None,
                authenticator_ref,
                processor_ref.clone(),
                processor_ref,
            )
            .await
        });
    }
}