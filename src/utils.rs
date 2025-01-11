use std::{
    fmt::Display,
    future::Future,
    ops::{FromResidual, Try},
    time::Duration,
};

use tokio::time::sleep;

pub async fn handle_retry<F, Fut, T>(mut task: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = (Option<T>, Option<f32>)>,
{
    let mut retries = 0;
    loop {
        let (result, retry_time) = task().await;
        match result {
            Some(x) => return x,
            None => {
                if retries >= 3 {
                    sleep(Duration::from_secs_f32(retry_time.unwrap_or(2.0))).await;
                }
                retries += 1;
            }
        };
    }
}

pub async fn handle_retry_indexed<F, Fut, T>(index: usize, task: F) -> (usize, T)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = (Option<T>, Option<f32>)>,
{
    (index, handle_retry(task).await)
}

pub trait StylizedText {
    fn to_colored(&self) -> String;
    fn to_title(&self) -> String;
}

impl StylizedText for &str {
    fn to_colored(&self) -> String {
        format!("\x1b[32m{}\x1b[0m", self)
    }
    fn to_title(&self) -> String {
        format!("{}\n{}", self, "=".repeat(self.len()))
    }
}

#[derive(Debug)]
pub enum SupportOption<T> {
    SupportedSome(T),
    SupportedNone,
    Unsupported,
}

impl<T: Clone> SupportOption<T> {
    pub fn to_result(&self) -> Result<Option<T>, String> {
        match self {
            Self::SupportedSome(x) => Ok(Some(x.clone())),
            Self::SupportedNone => Ok(None),
            Self::Unsupported => Err("Feature not supported at this time".to_string()),
        }
    }
}

impl<T> Try for SupportOption<T> {
    type Output = T;
    type Residual = SupportOption<std::convert::Infallible>;
    fn from_output(output: Self::Output) -> Self {
        Self::SupportedSome(output)
    }
    fn branch(self) -> std::ops::ControlFlow<Self::Residual, Self::Output> {
        match self {
            Self::SupportedSome(x) => std::ops::ControlFlow::Continue(x),
            Self::SupportedNone => std::ops::ControlFlow::Break(SupportOption::SupportedNone),
            Self::Unsupported => std::ops::ControlFlow::Break(SupportOption::Unsupported),
        }
    }
}
impl<T> FromResidual for SupportOption<T> {
    fn from_residual(residual: SupportOption<std::convert::Infallible>) -> Self {
        match residual {
            SupportOption::SupportedNone => SupportOption::SupportedNone,
            SupportOption::Unsupported => SupportOption::Unsupported,
            _ => unreachable!(),
        }
    }
}
impl<T> From<Option<T>> for SupportOption<T> {
    fn from(option: Option<T>) -> Self {
        match option {
            Some(value) => SupportOption::SupportedSome(value),
            None => SupportOption::SupportedNone,
        }
    }
}
impl<T: Clone> From<&Option<T>> for SupportOption<T> {
    fn from(option: &Option<T>) -> Self {
        match option {
            Some(value) => SupportOption::SupportedSome(value.clone()),
            None => SupportOption::SupportedNone,
        }
    }
}

pub trait ToSupported<T> {
    fn to_supported(&self) -> SupportOption<T>;
}

impl<T: Clone> ToSupported<T> for Option<T> {
    fn to_supported(&self) -> SupportOption<T> {
        self.into()
    }
}

#[derive(Debug)]
pub struct Table {
    rows: Vec<Vec<String>>,
    color_first_row: bool,
    spacing: usize,
}

impl From<Vec<Vec<String>>> for Table {
    fn from(value: Vec<Vec<String>>) -> Self {
        Self {
            rows: value,
            ..Default::default()
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            color_first_row: true,
            spacing: 2,
        }
    }
}

impl Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.rows.len() == 0 {
            return write!(f, "");
        }
        let row_count = self.rows.len();
        let col_count = self.rows[0].len();
        let mut col_widths: Vec<usize> = vec![0; col_count];
        for i in 0..row_count {
            for j in 0..col_count {
                col_widths[j] = (self.rows[i][j].len() + self.spacing).max(col_widths[j]);
            }
        }
        for i in 0..row_count {
            let row = self.rows[i]
                .iter()
                .enumerate()
                .map(|(j, r)| format!("{:width$}", r, width = col_widths[j]))
                .collect::<Vec<_>>()
                .join("");
            write!(
                f,
                "{}",
                if self.color_first_row && i == 0 {
                    row.as_str().to_colored()
                } else {
                    row
                }
            )?;
            if i != row_count - 1 {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}
