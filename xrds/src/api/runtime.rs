pub use xrds_runtime::Context;
pub use xrds_runtime::Object;
pub use xrds_runtime::RuntimeError;
pub use xrds_runtime::RuntimeHandler;
pub use xrds_runtime::RuntimeTarget;
pub use xrds_runtime::RuntimeWindowOptions;

pub struct Runtime {
    pub(crate) inner: xrds_runtime::Runtime,
}

#[derive(Default)]
pub struct RuntimeBuilder {
    pub(crate) application_name: String,
    pub(crate) target: RuntimeTarget,
    pub(crate) window_options: Option<RuntimeWindowOptions>,
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
        RuntimeBuilder::default()
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
                target: self.target,
                window_options: self.window_options,
            }),
        })
    }

    pub fn with_application_name(mut self, application_name: &str) -> Self {
        self.application_name = application_name.to_owned();
        self
    }

    pub fn with_target(mut self, target: RuntimeTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_window_options(mut self, window_options: RuntimeWindowOptions) -> Self {
        self.window_options = Some(window_options);
        self
    }
}
