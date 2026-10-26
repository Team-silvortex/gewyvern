//! Optional local liveness at existing execution boundaries, not a state machine.

use leselang_runtime_core::LocalExecutionScope;

pub(crate) struct CallFence<'scope> {
    scope: Option<&'scope LocalExecutionScope>,
}

impl<'scope> CallFence<'scope> {
    pub(crate) fn new(scope: Option<&'scope LocalExecutionScope>) -> Self {
        Self { scope }
    }

    pub(crate) fn close(&self) {
        if let Some(scope) = self.scope {
            scope.close();
        }
    }

    pub(crate) fn finish(mut self) {
        self.scope = None;
    }

    // Use only at a terminal slice boundary, not inside language error recovery.
    pub(crate) fn run<Value, Error>(
        scope: Option<&'scope LocalExecutionScope>,
        work: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        let fence = Self::new(scope);
        let output = work();
        if output.is_err() {
            fence.close();
        }
        fence.finish();
        output
    }
}

impl Drop for CallFence<'_> {
    fn drop(&mut self) {
        self.close();
    }
}
