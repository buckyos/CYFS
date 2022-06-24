use crate::Opcode;

/// Trap which indicates that an `ExternalOpcode` has to be handled.
pub type Trap = Opcode;

/// Capture represents the result of execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capture<E, T> {
	/// The machine has exited. It cannot be executed again.
	Exit(E),
	/// The machine has trapped. It is waiting for external information, and can
	/// be executed again.
	Trap(T),
}
