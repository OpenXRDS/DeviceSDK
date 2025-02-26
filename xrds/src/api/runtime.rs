pub use xrds_runtime::RuntimeError;
pub use xrds_runtime::RuntimeHandler;

pub struct Runtime {
    pub(crate) inner: xrds_runtime::Runtime,
}

pub struct RuntimeBuilder {
    pub(crate) application_name: String,
}

impl Runtime {
    /// Create xrds runtime
    ///
    /// This is an alias of 'Runtime::builder().build()'.
    #[inline]
    pub fn new() -> Result<Runtime, RuntimeError> {
        Self::builder().build()
    }

    #[inline]
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder {
            application_name: "".to_owned(),
        }
    }

    pub fn run<A>(self, app: A) -> Result<(), RuntimeError>
    where
        A: RuntimeHandler + Send + Sync + 'static,
    {
        self.inner.run_block(app).expect("Unexpected error");
        Ok(())
    }
}

impl RuntimeBuilder {
    pub fn build(self) -> Result<Runtime, RuntimeError> {
        Ok(Runtime {
            inner: xrds_runtime::Runtime::new(xrds_runtime::RuntimeParameters {
                app_name: self.application_name,
            }),
        })
    }
}
