pub mod handlers;
mod response;
mod routes;
pub mod server;
pub use handlers::websocket::WebSocketHandler;
pub use handlers::websocket_message::WebsocketMessageHandler;
pub use server::WebServer;

use crate::errors::LockError;
use crate::web::response::ApiResponse;
use warp::Rejection;

pub async fn handle_rejection(
    err: Rejection,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let (code, message) = if let Some(lock_error) = err.find::<LockError>() {
        (500, lock_error.to_string())
    } else {
        (500, "内部服务器错误".to_string())
    };

    Ok(ApiResponse::<()>::error(code, message)
        .into_response()
        .unwrap())
}
