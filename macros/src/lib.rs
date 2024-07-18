macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(x) => x,
            Err(e) => return e.into_compile_error().into(),
        }
    };
}
