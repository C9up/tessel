//! A bounded binary encoding, shared by the message format (story 02-05) and the local
//! database (story 02-06): big-endian integers and length-prefixed fields. The reader
//! never reads beyond what remains, so a truncated or forged input is an error, never a
//! panic.

/// What the encoding can reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodecError {
    /// A field is longer than its length prefix allows.
    TooLong,
    /// The input is truncated or violates the encoding.
    Malformed,
}

/// Writes fields one after the other.
#[derive(Default)]
pub(crate) struct Writer(pub(crate) Vec<u8>);

impl Writer {
    pub(crate) fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    pub(crate) fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    pub(crate) fn vec8(&mut self, bytes: &[u8]) -> Result<(), CodecError> {
        self.u8(u8::try_from(bytes.len()).map_err(|_| CodecError::TooLong)?);
        self.bytes(bytes);
        Ok(())
    }

    pub(crate) fn vec16(&mut self, bytes: &[u8]) -> Result<(), CodecError> {
        self.u16(u16::try_from(bytes.len()).map_err(|_| CodecError::TooLong)?);
        self.bytes(bytes);
        Ok(())
    }

    pub(crate) fn vec32(&mut self, bytes: &[u8]) -> Result<(), CodecError> {
        self.u32(u32::try_from(bytes.len()).map_err(|_| CodecError::TooLong)?);
        self.bytes(bytes);
        Ok(())
    }
}

/// Reads fields one after the other, without ever reading beyond the input.
pub(crate) struct Reader<'a>(pub(crate) &'a [u8]);

impl<'a> Reader<'a> {
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        if n > self.0.len() {
            return Err(CodecError::Malformed);
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Ok(head)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        self.take(N)?.try_into().map_err(|_| CodecError::Malformed)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, CodecError> {
        self.array::<1>().map(|[b]| b)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, CodecError> {
        self.array().map(u16::from_be_bytes)
    }

    pub(crate) fn u32(&mut self) -> Result<u32, CodecError> {
        self.array().map(u32::from_be_bytes)
    }

    pub(crate) fn u64(&mut self) -> Result<u64, CodecError> {
        self.array().map(u64::from_be_bytes)
    }

    pub(crate) fn bool(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CodecError::Malformed),
        }
    }

    pub(crate) fn vec8(&mut self) -> Result<&'a [u8], CodecError> {
        let len = self.u8()?;
        self.take(usize::from(len))
    }

    pub(crate) fn vec16(&mut self) -> Result<&'a [u8], CodecError> {
        let len = self.u16()?;
        self.take(usize::from(len))
    }

    pub(crate) fn vec32(&mut self) -> Result<&'a [u8], CodecError> {
        let len = self.u32()?;
        self.take(usize::try_from(len).map_err(|_| CodecError::Malformed)?)
    }

    pub(crate) fn string8(&mut self) -> Result<String, CodecError> {
        String::from_utf8(self.vec8()?.to_vec()).map_err(|_| CodecError::Malformed)
    }

    pub(crate) fn string16(&mut self) -> Result<String, CodecError> {
        String::from_utf8(self.vec16()?.to_vec()).map_err(|_| CodecError::Malformed)
    }

    /// The input is fully consumed.
    pub(crate) fn finish(&self) -> Result<(), CodecError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(CodecError::Malformed)
        }
    }
}
