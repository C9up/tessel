//! A native peer for the WASM ↔ native interoperability test (story 02-12,
//! `packages/core-web/src/interop.test.ts`): devices of the core built natively, driven
//! line by line on stdin, answering on stdout. Bytes travel in hex.
//!
//! ```text
//! device <name>                       -> ok <certificate>
//! key-package <name>                  -> ok <key package>
//! join <name> <welcome>               -> ok <group>
//! add <name> <group> <key package>    -> ok <commit> <welcome>
//! send <name> <group> <utf-8 text>    -> ok <encrypted>
//! process <name> <message>            -> ok message <utf-8 text> | ok commit <epoch> | ok proposal
//! members <name> <group>              -> ok <count>
//! anything failing                    -> err <reason>
//! ```

use std::collections::HashMap;
use std::io::{BufRead as _, Write as _};

use tessel_core::{
    AccountId, Body, Client, GroupId, Incoming, Message, StoreKey, verify_key_package,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd hex length".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Default)]
struct Peer {
    devices: HashMap<String, Client>,
}

impl Peer {
    fn device(&mut self, name: &str) -> Result<&mut Client, String> {
        self.devices
            .get_mut(name)
            .ok_or_else(|| format!("no device {name}"))
    }

    fn run(&mut self, line: &str) -> Result<String, String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        let arg = |i: usize| {
            words
                .get(i)
                .copied()
                .ok_or_else(|| format!("missing argument {i}"))
        };
        let e = |error: &dyn std::fmt::Display| error.to_string();
        match words.first().copied() {
            Some("device") => {
                let name = arg(1)?;
                let account =
                    AccountId::parse(&format!("{name}@tessel.example")).map_err(|x| e(&x))?;
                let key = StoreKey::generate().map_err(|x| e(&x))?;
                let client = Client::create_account(&account, &key).map_err(|x| e(&x))?;
                let certificate = client
                    .device()
                    .certificate()
                    .to_bytes()
                    .map_err(|x| e(&x))?;
                self.devices.insert(name.to_owned(), client);
                Ok(hex(&certificate))
            }
            Some("key-package") => {
                let update = self
                    .device(arg(1)?)?
                    .refresh_key_packages(now())
                    .map_err(|x| e(&x))?;
                let package = update.publish.first().ok_or("no key package")?;
                Ok(hex(package))
            }
            Some("join") => {
                let welcome = unhex(arg(2)?)?;
                let group = self
                    .device(arg(1)?)?
                    .join_group(&welcome)
                    .map_err(|x| e(&x))?;
                Ok(hex(group.as_bytes()))
            }
            Some("add") => {
                let group = GroupId::from_bytes(&unhex(arg(2)?)?);
                let package = verify_key_package(&unhex(arg(3)?)?).map_err(|x| e(&x))?;
                let client = self.device(arg(1)?)?;
                let outgoing = client.add_devices(&group, &[package]).map_err(|x| e(&x))?;
                client.commit_accepted(&group).map_err(|x| e(&x))?;
                let welcome = outgoing.welcome.ok_or("no welcome")?;
                Ok(format!("{} {}", hex(&outgoing.commit), hex(&welcome)))
            }
            Some("send") => {
                let group = GroupId::from_bytes(&unhex(arg(2)?)?);
                let text = String::from_utf8(unhex(arg(3)?)?).map_err(|x| e(&x))?;
                let message = Message::new(
                    1_000,
                    Body::Text {
                        text,
                        reply_to: None,
                    },
                )
                .map_err(|x| e(&x))?;
                let client = self.device(arg(1)?)?;
                let send = client.queue_message(&group, &message).map_err(|x| e(&x))?;
                let outbound = client
                    .next_outbound(&group)
                    .map_err(|x| e(&x))?
                    .ok_or("empty queue")?;
                client.outbound_sent(&group, &send).map_err(|x| e(&x))?;
                Ok(hex(&outbound.encrypted))
            }
            Some("process") => {
                let message = unhex(arg(2)?)?;
                let client = self.device(arg(1)?)?;
                match client.process(&message).map_err(|x| e(&x))? {
                    Incoming::Message { message, .. } => match message.map_err(|x| e(&x))?.body {
                        Body::Text { text, .. } => Ok(format!("message {}", hex(text.as_bytes()))),
                        _ => Ok("message".into()),
                    },
                    Incoming::Commit { group, .. } => Ok(format!(
                        "commit {}",
                        client.epoch(&group).map_err(|x| e(&x))?
                    )),
                    Incoming::Proposal { .. } => Ok("proposal".into()),
                }
            }
            Some("members") => {
                let group = GroupId::from_bytes(&unhex(arg(2)?)?);
                let members = self.device(arg(1)?)?.members(&group).map_err(|x| e(&x))?;
                Ok(members.len().to_string())
            }
            _ => Err(format!("unknown command: {line}")),
        }
    }
}

fn main() {
    let mut peer = Peer::default();
    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let answer = match peer.run(&line) {
            Ok(value) => format!("ok {value}"),
            Err(reason) => format!("err {}", reason.replace('\n', " ")),
        };
        let mut out = stdout.lock();
        if writeln!(out, "{answer}")
            .and_then(|()| out.flush())
            .is_err()
        {
            break;
        }
    }
}
