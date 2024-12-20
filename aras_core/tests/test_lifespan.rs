use std::marker::PhantomData;

use aras_core::{ASGICallable, Application, ASGIMessage, ApplicationFactory, LifespanHandler, State, SendFn, ReceiveFn, Scope};


#[derive(Clone, Debug)]
struct MockState;
impl State for MockState {}

#[derive(Clone, Debug)]
struct TestASGIApp;

impl ASGICallable<MockState> for TestASGIApp {
    async fn call(&self, scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> aras_core::Result<()>{
        if let Scope::Lifespan(_) = scope {
            let recv = receive().await?;
            if let ASGIMessage::Startup(_) = recv {
                send(ASGIMessage::new_startup_complete()).await?;
                return Ok(())
            }
            return Err(aras_core::Error::custom("Invalid message"))
        }
        Err(aras_core::Error::custom("Invalid scope"))
    }
}

fn create_application() -> Application<MockState, TestASGIApp> {
    ApplicationFactory::new(TestASGIApp {}, PhantomData).build()
}

#[tokio::test]
async fn test_lifespan_startup() {
    let mut lifespan_handler = LifespanHandler::new(create_application());
    let result = lifespan_handler.startup(MockState{}).await;
    println!("{:?}", result);
    assert!(result.is_ok());
}