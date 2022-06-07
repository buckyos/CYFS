use crate::{Runtime, Handler, ExitFatal};

/// Interrupt resolution.
pub enum Resolve<'a, 'config, H: Handler> {
	/// Create interrupt resolution.
	Create(H::CreateInterrupt, ResolveCreate<'a, 'config>),
	/// Call interrupt resolution.
	Call(H::CallInterrupt, ResolveCall<'a, 'config>),
}

/// Create interrupt resolution.
pub struct ResolveCreate<'a, 'config> {
	runtime: &'a mut Runtime<'config>,
}

impl<'a, 'config> ResolveCreate<'a, 'config> {
	pub(crate) fn new(runtime: &'a mut Runtime<'config>) -> Self {
		Self { runtime }
	}
}

impl<'a, 'config> Drop for ResolveCreate<'a, 'config> {
	fn drop(&mut self) {
		self.runtime.status = Err(ExitFatal::UnhandledInterrupt.into());
		self.runtime.machine.exit(ExitFatal::UnhandledInterrupt.into());
	}
}

/// Call interrupt resolution.
pub struct ResolveCall<'a, 'config> {
	runtime: &'a mut Runtime<'config>,
}

impl<'a, 'config> ResolveCall<'a, 'config> {
	pub(crate) fn new(runtime: &'a mut Runtime<'config>) -> Self {
		Self { runtime }
	}
}

impl<'a, 'config> Drop for ResolveCall<'a, 'config> {
	fn drop(&mut self) {
		self.runtime.status = Err(ExitFatal::UnhandledInterrupt.into());
		self.runtime.machine.exit(ExitFatal::UnhandledInterrupt.into());
	}
}
