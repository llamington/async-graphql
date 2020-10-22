use async_graphql::{Data, ObjectType, Result, Schema, SubscriptionType};
use futures_util::{future, StreamExt};
use warp::filters::ws;
use warp::{Filter, Rejection, Reply};

/// GraphQL subscription filter
///
/// # Examples
///
/// ```no_run
/// use async_graphql::*;
/// use async_graphql_warp::*;
/// use warp::Filter;
/// use futures_util::stream::{Stream, StreamExt};
/// use std::time::Duration;
///
/// struct QueryRoot;
///
/// #[Object]
/// impl QueryRoot {
///     async fn value(&self) -> i32 {
///         // A GraphQL Object type must define one or more fields.
///         100
///     }
/// }
///
/// struct SubscriptionRoot;
///
/// #[Subscription]
/// impl SubscriptionRoot {
///     async fn tick(&self) -> impl Stream<Item = String> {
///         tokio::time::interval(Duration::from_secs(1)).map(|n| format!("{}", n.elapsed().as_secs_f32()))
///     }
/// }
///
/// tokio::runtime::Runtime::new().unwrap().block_on(async {
///     let schema = Schema::new(QueryRoot, EmptyMutation, SubscriptionRoot);
///     let filter = async_graphql_warp::graphql_subscription(schema)
///         .or(warp::any().map(|| "Hello, World!"));
///     warp::serve(filter).run(([0, 0, 0, 0], 8000)).await;
/// });
/// ```
pub fn graphql_subscription<Query, Mutation, Subscription>(
    schema: Schema<Query, Mutation, Subscription>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    Query: ObjectType + Sync + Send + 'static,
    Mutation: ObjectType + Sync + Send + 'static,
    Subscription: SubscriptionType + Send + Sync + 'static,
{
    graphql_subscription_with_data::<_, _, _, fn(serde_json::Value) -> Result<Data>>(schema, None)
}

/// GraphQL subscription filter
///
/// Specifies that a function converts the init payload to data.
pub fn graphql_subscription_with_data<Query, Mutation, Subscription, F>(
    schema: Schema<Query, Mutation, Subscription>,
    initializer: Option<F>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    Query: ObjectType + Sync + Send + 'static,
    Mutation: ObjectType + Sync + Send + 'static,
    Subscription: SubscriptionType + Send + Sync + 'static,
    F: FnOnce(serde_json::Value) -> Result<Data> + Send + Sync + Clone + 'static,
{
    warp::ws().map(move |ws: ws::Ws| {
        let schema = schema.clone();
        let initializer = initializer.clone();

        let reply = ws.on_upgrade(move |websocket| {
            let (ws_sender, ws_receiver) = websocket.split();

            async move {
                let _ = async_graphql::http::WebSocket::with_data(
                    schema,
                    ws_receiver
                        .take_while(|msg| future::ready(msg.is_ok()))
                        .map(Result::unwrap)
                        .map(ws::Message::into_bytes),
                    initializer,
                )
                .map(ws::Message::text)
                .map(Ok)
                .forward(ws_sender)
                .await;
            }
        });
        warp::reply::with_header(reply, "Sec-WebSocket-Protocol", "graphql-ws")
    })
}
