use std::collections::BTreeMap;

use bytes::{Buf, BufMut, BytesMut};

use super::codec;
use super::Message;

/// postgresql wire protocol startup message, sent by frontend
/// the strings are null-ternimated string, which is a string
/// terminated by a zero byte.
/// the key-value parameter pairs are terminated by a zero byte, too.
///
#[derive(Getters, Setters, MutGetters, PartialEq, Eq, Debug, new)]
#[getset(get = "pub", set = "pub", get_mut = "pub")]
pub struct Startup {
    #[new(value = "3")]
    protocol_number_major: u16,
    #[new(value = "0")]
    protocol_number_minor: u16,
    #[new(default)]
    parameters: BTreeMap<String, String>,
}

impl Default for Startup {
    fn default() -> Startup {
        Startup::new()
    }
}

impl Message for Startup {
    fn message_length(&self) -> usize {
        let param_length = self
            .parameters
            .iter()
            .map(|(k, v)| k.as_bytes().len() + v.as_bytes().len() + 2)
            .sum::<usize>();
        // length:4 + protocol_number:4 + param.len + nullbyte:1
        9 + param_length
    }

    fn encode_body(&self, buf: &mut BytesMut) -> std::io::Result<()> {
        // version number
        buf.put_u16(self.protocol_number_major);
        buf.put_u16(self.protocol_number_minor);

        // parameters
        for (k, v) in self.parameters.iter() {
            codec::put_cstring(buf, &k);
            codec::put_cstring(buf, &v);
        }
        // ends with empty cstring, a \0
        codec::put_cstring(buf, "");

        Ok(())
    }

    fn decode_body(buf: &mut BytesMut) -> std::io::Result<Self> {
        let mut msg = Startup::default();
        // parse
        msg.set_protocol_number_major(buf.get_u16());
        msg.set_protocol_number_minor(buf.get_u16());

        // end by reading the last \0
        while let Some(key) = codec::get_cstring(buf) {
            let value = codec::get_cstring(buf).unwrap_or_else(|| "".to_owned());
            msg.parameters_mut().insert(key, value);
        }

        Ok(msg)
    }
}

/// authentication response family, sent by backend
#[derive(PartialEq, Eq, Debug)]
pub enum Authentication {
    Ok,                // code 0
    CleartextPassword, // code 3
    KerberosV5,        // code 2
    MD5Password([u8; 4]), // code 5, with 4 bytes of md5 salt

                       // TODO: more types
                       // AuthenticationSCMCredential
                       //
                       // AuthenticationGSS
                       // AuthenticationGSSContinue
                       // AuthenticationSSPI
                       // AuthenticationSASL
                       // AuthenticationSASLContinue
                       // AuthenticationSASLFinal
}

impl Message for Authentication {
    #[inline]
    fn message_type() -> Option<u8> {
        Some(b'R')
    }

    #[inline]
    fn message_length(&self) -> usize {
        match self {
            Authentication::Ok | Authentication::CleartextPassword | Authentication::KerberosV5 => {
                8
            }
            Authentication::MD5Password(_) => 12,
        }
    }

    fn encode_body(&self, buf: &mut BytesMut) -> std::io::Result<()> {
        match self {
            Authentication::Ok => buf.put_i32(0),
            Authentication::CleartextPassword => buf.put_i32(3),
            Authentication::KerberosV5 => buf.put_i32(2),
            Authentication::MD5Password(salt) => {
                buf.put_i32(5);
                buf.put_slice(salt.as_ref());
            }
        }
        Ok(())
    }

    fn decode_body(buf: &mut BytesMut) -> std::io::Result<Self> {
        let code = buf.get_i32();
        let msg = match code {
            0 => Authentication::Ok,
            2 => Authentication::KerberosV5,
            3 => Authentication::CleartextPassword,
            5 => {
                let mut salt = buf.split_to(4);
                let mut salt_array = [0u8; 4];
                salt.copy_to_slice(&mut salt_array);
                Authentication::MD5Password(salt_array)
            }
            _ => unreachable!(),
        };

        Ok(msg)
    }
}

/// password packet sent from frontend
#[derive(Getters, Setters, MutGetters, PartialEq, Eq, Debug, new)]
#[getset(get = "pub", set = "pub", get_mut = "pub")]
pub struct Password {
    password: String,
}

impl Message for Password {
    #[inline]
    fn message_type() -> Option<u8> {
        Some(b'p')
    }

    fn message_length(&self) -> usize {
        5 + self.password.as_bytes().len()
    }

    fn encode_body(&self, buf: &mut BytesMut) -> std::io::Result<()> {
        codec::put_cstring(buf, &self.password);

        Ok(())
    }

    fn decode_body(buf: &mut BytesMut) -> std::io::Result<Self> {
        let pass = codec::get_cstring(buf).unwrap_or_else(|| "".to_owned());

        Ok(Password::new(pass))
    }
}

/// parameter ack sent from backend after authentication success
#[derive(Getters, Setters, MutGetters, PartialEq, Eq, Debug, new)]
#[getset(get = "pub", set = "pub", get_mut = "pub")]
pub struct ParameterStatus {
    name: String,
    value: String,
}

impl Message for ParameterStatus {
    #[inline]
    fn message_type() -> Option<u8> {
        Some(b'S')
    }

    fn message_length(&self) -> usize {
        4 + 2 + self.name.as_bytes().len() + self.value.as_bytes().len()
    }

    fn encode_body(&self, buf: &mut BytesMut) -> std::io::Result<()> {
        codec::put_cstring(buf, &self.name);
        codec::put_cstring(buf, &self.value);

        Ok(())
    }

    fn decode_body(buf: &mut BytesMut) -> std::io::Result<Self> {
        let name = codec::get_cstring(buf).unwrap_or_else(|| "".to_owned());
        let value = codec::get_cstring(buf).unwrap_or_else(|| "".to_owned());

        Ok(ParameterStatus::new(name, value))
    }
}

/// `BackendKeyData` message, sent from backend to frontend for issuing
/// `CancelRequestMessage`
#[derive(Getters, Setters, MutGetters, PartialEq, Eq, Debug, new)]
#[getset(get = "pub", set = "pub", get_mut = "pub")]
pub struct BackendKeyData {
    pid: i32,
    secret_key: i32,
}

impl Message for BackendKeyData {
    #[inline]
    fn message_type() -> Option<u8> {
        Some(b'K')
    }

    #[inline]
    fn message_length(&self) -> usize {
        12
    }

    fn encode_body(&self, buf: &mut BytesMut) -> std::io::Result<()> {
        buf.put_i32(self.pid);
        buf.put_i32(self.secret_key);

        Ok(())
    }

    fn decode_body(buf: &mut BytesMut) -> std::io::Result<Self> {
        let pid = buf.get_i32();
        let secret_key = buf.get_i32();

        Ok(BackendKeyData { pid, secret_key })
    }
}
