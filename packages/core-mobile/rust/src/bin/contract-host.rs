//! The phone's host on a computer, for the contract tests of `@tessel/core-mobile`
//! (story 02-13): the same [`Host`] the JSI module calls, driven over two FIFOs with
//! the same synchronous call. Never built into the app (feature `contract-host`).
//!
//! Requests: method length (u32 LE), method, arguments length (u32 LE), arguments.
//! Answers: status (0 or 1, as `tessel_call`), length (u32 LE), bytes.
//! The platform's seal is a stand-in: a computer has no Keystore.

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};

use tessel_mobile::Host;

/// A stand-in for the Keystore: a marker and the bytes flipped.
fn seal(unseal: bool, input: &[u8]) -> Option<Vec<u8>> {
    if unseal {
        let (marker, rest) = input.split_first()?;
        (*marker == 0xEE).then(|| rest.iter().map(|b| b ^ 0x5A).collect())
    } else {
        Some(
            std::iter::once(0xEE)
                .chain(input.iter().map(|b| b ^ 0x5A))
                .collect(),
        )
    }
}

/// One length-prefixed frame; `None` at the end of the requests.
fn frame(input: &mut File) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    let mut length = [0_u8; 4];
    match input.read_exact(&mut length) {
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        other => other?,
    }
    let mut bytes = vec![0_u8; usize::try_from(u32::from_le_bytes(length))?];
    input.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let [_, dir, requests, answers] = args.as_slice() else {
        return Err("usage: contract-host <database dir> <requests fifo> <answers fifo>".into());
    };
    // In this order: the test opens the requests for writing, then the answers for reading.
    let mut requests = File::open(requests)?;
    let mut answers = OpenOptions::new().write(true).open(answers)?;
    let mut host = Host::new(dir, Box::new(seal));
    while let Some(method) = frame(&mut requests)? {
        let input = frame(&mut requests)?.ok_or("a request without its arguments")?;
        let (status, bytes) = match host.call(&String::from_utf8(method)?, &input) {
            Ok(answer) => (0_u8, answer),
            Err(error) => (1, format!("{}\n{}", error.code, error.message).into_bytes()),
        };
        answers.write_all(&[status])?;
        answers.write_all(&u32::try_from(bytes.len())?.to_le_bytes())?;
        answers.write_all(&bytes)?;
        answers.flush()?;
    }
    Ok(())
}
