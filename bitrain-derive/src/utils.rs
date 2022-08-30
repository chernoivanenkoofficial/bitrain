pub use syn::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub trait ReduceErrors {
    type Output;
    fn reduce_errors(self) -> Result<Self::Output>;
}

impl<T, I: Iterator<Item = Result<T>>> ReduceErrors for I {
    type Output = Vec<T>;

    fn reduce_errors(self) -> Result<Self::Output> {
        let (ok, failed): (Vec<_>, Vec<_>) = self.partition(Result::is_ok);

        if failed.len() > 0 {
            let err = failed
                .into_iter()
                .map(|err| err.err().unwrap())
                .reduce(|mut a, b| {
                    a.combine(b);
                    a
                })
                .unwrap();

            Err(err)
        } else {
            let ok = ok.into_iter().map(Result::unwrap).collect();
            Ok(ok)
        }
    }
}