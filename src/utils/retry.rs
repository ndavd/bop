use reqwest::{Response, StatusCode};
use std::{future::Future, time::Duration};
use tokio::time::sleep;

pub fn get_retry_time(response: &Response) -> Option<f32> {
    if response.status() != StatusCode::TOO_MANY_REQUESTS {
        return None;
    }
    response
        .headers()
        .get("retry-after")
        .and_then(|x| x.to_str().ok())
        .and_then(|x| x.parse().ok())
}

pub async fn handle_retry<F, Fut, T>(mut task: F) -> T
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = (Option<T>, Option<f32>)>,
{
    let mut retries = 0;
    // NOTE: Index that is used to change RPC
    let mut index = 0;
    loop {
        let (result, retry_time) = task(index).await;
        match result {
            Some(x) => return x,
            None => {
                if retries >= 3 {
                    sleep(Duration::from_secs_f32(retry_time.unwrap_or(2.0))).await;
                }
                retries += 1;
                index += 1;
            }
        };
    }
}

pub async fn handle_retry_indexed<F, Fut, T>(index: usize, task: F) -> (usize, T)
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = (Option<T>, Option<f32>)>,
{
    (index, handle_retry(task).await)
}
