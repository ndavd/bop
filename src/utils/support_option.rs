use std::ops::{FromResidual, Try};

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
