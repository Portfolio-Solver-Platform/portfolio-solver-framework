pub trait IsCancelled {
    fn is_cancelled(&self) -> bool;
}

pub trait IsErrorCancelled {
    fn is_error_cancelled(&self) -> bool;
}

impl<T, E> IsErrorCancelled for Result<T, E>
where
    E: IsCancelled,
{
    fn is_error_cancelled(&self) -> bool {
        match self {
            Ok(_) => false,
            Err(e) => e.is_cancelled(),
        }
    }
}
