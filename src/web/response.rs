use serde::Serialize;
use warp::{Rejection, Reply};

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: u16,
    pub message: String,
    pub data: Option<T>,
}

#[derive(Serialize)]
pub struct ListResponse<T: Serialize> {
    list: Vec<T>,
    total: usize,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn new(code: u16, message: String, data: Option<T>) -> Self {
        Self {
            code,
            message,
            data,
        }
    }

    pub fn success(data: T) -> Self {
        Self::new(200, "成功".to_string(), Some(data))
    }

    pub fn success_list(data: Vec<T>) -> ApiResponse<ListResponse<T>> {
        let total = data.len();
        let list_response = ListResponse { list: data, total };
        ApiResponse::new(200, "成功".to_string(), Some(list_response))
    }

    pub fn error(code: u16, message: String) -> Self {
        Self::new(code, message, None)
    }

    pub fn into_response(self) -> Result<impl Reply, Rejection> {
        Ok(warp::reply::with_status(
            warp::reply::json(&self),
            warp::http::StatusCode::from_u16(self.code).unwrap_or(warp::http::StatusCode::OK),
        ))
    }
}
