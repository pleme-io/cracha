// crachá-controller — library surface.
//
// Both the controller binary and the cracha-api binary import this
// to access the shared SharedIndex type. The controller binary
// owns the reconcile loops; the api binary reads the index.

pub mod index;
pub mod reconcile;

pub use index::{new_shared_index, replace, SharedIndex};
pub use reconcile::{run, Context};
