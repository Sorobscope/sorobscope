//! Whether a command found the thing it was asked for.
//!
//! This exists so a script can tell "the entry isn't there" from "the tool broke". Both
//! print to the terminal and both are non-zero, but conflating them would mean a failed
//! lookup and an unreachable endpoint look identical in a shell pipeline, which is a poor
//! trade for a tool whose whole point is being scriptable.
pub enum Outcome {
    Found,
    NotFound,
}
