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
    let mut rpc_index = 0;
    let maximum_retry_time_secs = 1.0;
    loop {
        let (result, retry_time) = task(rpc_index).await;
        match result {
            Some(x) => {
                return x;
            }
            None => {
                if retries >= 2 {
                    if let Some(retry_time) = retry_time {
                        sleep(Duration::from_secs_f32(
                            retry_time.min(maximum_retry_time_secs),
                        ))
                        .await;
                    }
                    rpc_index += 1;
                }
                retries += 1;
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
