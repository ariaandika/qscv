use bytes::{BufMut, Bytes};

use super::error::{ProtocolError, protocol_err};

pub trait UsizeExt {
    /// length is usize in rust, while postgres want i32,
    /// this will panic when overflow instead of wrapping
    fn to_i32(self) -> i32;
    /// length is usize in rust, while sometime postgres want i16,
    /// this will panic when overflow instead of wrapping
    fn to_i16(self) -> i16;
}

impl UsizeExt for usize {
    fn to_i32(self) -> i32 {
        match i32::try_from(self) {
            Ok(ok) => ok,
            Err(err) => panic!("message size too large for protocol: {err}"),
        }
    }

    fn to_i16(self) -> i16 {
        match i16::try_from(self) {
            Ok(ok) => ok,
            Err(err) => panic!("message size too large for protocol: {err}"),
        }
    }
}

pub trait StrExt {
    /// postgres String must be nul terminated
    fn nul_string_len(&self) -> i32;
}

impl StrExt for str {
    fn nul_string_len(&self) -> i32 {
        self.len().to_i32() + 1/* nul */
    }
}

pub trait BufMutExt {
    /// postgres String must be nul terminated
    fn put_nul_string(&mut self, string: &str);
}

impl<B: BufMut> BufMutExt for B {
    fn put_nul_string(&mut self, string: &str) {
        self.put(string.as_bytes());
        self.put_u8(b'\0');
    }
}

pub trait BytesExt {
    fn get_nul_string(&mut self) -> Result<String,ProtocolError>;
}

impl BytesExt for Bytes {
    fn get_nul_string(&mut self) -> Result<String,ProtocolError> {
        let Some(end) = self.iter().position(|e|matches!(e,b'\0')) else {
            return Err(protocol_err!("no nul termination for string"))
        };
        match String::from_utf8(self.split_to(end).into()) {
            Ok(ok) => Ok(ok),
            Err(err) => Err(protocol_err!("non UTF-8 string: {err}")),
        }
    }
}

