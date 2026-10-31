// SPDX-License-Identifier: Apache-2.0

//! Fresh bytes for a consent question's code (work order 16 part E, SR 10).
//!
//! A file read, not a spawn: the system's random device is opened read-only,
//! checked on the handle to be a character device, and three bytes are read
//! with `read_exact` — never a read to the end, which on that device never
//! ends. Any failure is `None`, and a question with no code is not asked, so
//! nothing here falls back to a fixed code.

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;

/// The system's random device. An absolute path, never looked up.
const RANDOM_DEVICE: &str = "/dev/urandom";

/// Three fresh bytes from the system's random device, or `None` when it cannot
/// be opened, is not a character device, or gives fewer than three.
#[must_use]
pub fn fresh_bytes() -> Option<[u8; 3]> {
    bytes_from(Path::new(RANDOM_DEVICE))
}

fn bytes_from(path: &Path) -> Option<[u8; 3]> {
    let mut file = File::open(path).ok()?;
    if !file.metadata().ok()?.file_type().is_char_device() {
        return None;
    }
    let mut bytes = [0u8; 3];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_reads_three_bytes_or_answers_none() {
        assert!(fresh_bytes().is_some(), "the random device gives bytes");
        let missing = std::env::temp_dir().join(format!("mpb-no-device-{}", std::process::id()));
        assert_eq!(bytes_from(&missing), None, "a missing device");
        // A regular file holding enough bytes is not the device.
        let regular = std::env::temp_dir().join(format!("mpb-regular-{}", std::process::id()));
        std::fs::write(&regular, [1u8, 2, 3, 4]).expect("scratch");
        assert_eq!(bytes_from(&regular), None, "a regular file");
        // A character device that gives no bytes: three are required.
        assert_eq!(bytes_from(Path::new("/dev/null")), None, "an empty device");
        std::fs::remove_file(&regular).ok();
    }

    #[test]
    fn two_draws_differ() {
        // Not a randomness test: a reader that returned a constant would
        // pass every other test, and two draws of 24 bits collide with
        // probability 2^-24.
        assert_ne!(fresh_bytes(), fresh_bytes());
    }
}
