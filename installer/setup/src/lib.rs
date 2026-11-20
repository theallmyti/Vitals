//! Everything the Vitals installer does besides drawing its window, as a library so it can be
//! tested. (The test runner is named after this library, `vitals_core`: a name with "setup" or
//! "install" in it makes Windows demand admin rights for the test program, error 740.)

pub mod assets;
#[cfg(windows)]
pub mod ops;
#[cfg(windows)]
pub mod pair;
pub mod payload;
pub mod text;
