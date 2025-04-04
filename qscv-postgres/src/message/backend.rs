//! Postgres Backend Messages
use bytes::{Buf, Bytes};

use super::{
    error::{DatabaseError, ProtocolError},
    ext::BytesExt,
};
use crate::row_buffer::RowBuffer;

/// A type that can be decoded into postgres backend message
pub trait BackendProtocol: Sized {
    fn decode(msgtype: u8, body: Bytes) -> Result<Self,ProtocolError>;
}

macro_rules! assert_msgtype {
    ($self:ident,$typ:ident) => {
        if $self::MSGTYPE != $typ {
            return Err(ProtocolError::unexpected(stringify!($self),$self::MSGTYPE,$typ))
        }
    };
}

/// Postgres backend messages
#[derive(Debug)]
pub enum BackendMessage {
    Authentication(Authentication),
    BackendKeyData(BackendKeyData),
    NoticeResponse(NoticeResponse),
    ErrorResponse(ErrorResponse),
    ParameterStatus(ParameterStatus),
    ReadyForQuery(ReadyForQuery),
    RowDescription(RowDescription),
    DataRow(DataRow),
    CommandComplete(CommandComplete),
    ParseComplete(ParseComplete),
    BindComplete(BindComplete),
    CloseComplete(CloseComplete),
}

impl BackendProtocol for BackendMessage {
    fn decode(msgtype: u8, body: Bytes) -> Result<Self, ProtocolError> {
        macro_rules! match_type {
            ($($name:ident,)*) => {
                match msgtype {
                    $($name::MSGTYPE => Self::$name(<$name as BackendProtocol>::decode(msgtype, body)?),)*
                    _ => return Err(ProtocolError::unknown(msgtype)),
                }
            };
        }
        let message = match_type! {
            Authentication, BackendKeyData, NoticeResponse, ErrorResponse, ParameterStatus,
            ReadyForQuery, RowDescription, DataRow, CommandComplete, ParseComplete, BindComplete, CloseComplete,
        };
        Ok(message)
    }
}

impl BackendMessage {
    pub fn try_dberror(self) -> Result<Self, DatabaseError> {
        match self {
            Self::ErrorResponse(err) => Err(err.to_db_error()),
            ok => Ok(ok),
        }
    }

    pub fn msgtype(&self) -> u8 {
        macro_rules! match_type {
            ($($name:ident,)*) => {
                match self {
                    $(Self::$name(_) => $name::MSGTYPE,)*
                }
            };
        }
        match_type! {
            Authentication, BackendKeyData, NoticeResponse, ErrorResponse, ParameterStatus,
            ReadyForQuery, RowDescription, DataRow, CommandComplete, ParseComplete, BindComplete, CloseComplete,
        }
    }
}

/// Identifies the message as an authentication request.
#[derive(Debug)]
pub enum Authentication {
    /// Int32(0) Specifies that the authentication was successful.
    Ok,
    /// Int32(2) Specifies that Kerberos V5 authentication is required.
    KerberosV5,
    /// Int32(3) Specifies that a clear-text password is required.
    CleartextPassword,
    /// Int32(5) Specifies that an MD5-encrypted password is required.
    /// Byte4 The salt to use when encrypting the password.
    MD5Password {
        salt: u32
    },
    /// Int32(7) Specifies that GSSAPI authentication is required.
    GSS,
    /// Int32(9) Specifies that SSPI authentication is required.
    SSPI,
    /// Int32(10) Specifies that SASL authentication is required.
    ///   The message body is a list of SASL authentication mechanisms,
    ///   in the server's order of preference. A zero byte is required
    ///   as terminator after the last authentication mechanism name.
    ///   For each mechanism, there is the following:
    /// String Name of a SASL authentication mechanism.
    /// TODO: SASL not yet supported
    /// there are more protocol for SASL control flow
    SASL,
}

impl Authentication {
    pub const MSGTYPE: u8 = b'R';
}

impl BackendProtocol for Authentication {
    fn decode(msgtype: u8, mut body: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(Authentication,msgtype);
        let auth = match body.get_i32() {
            0 => Authentication::Ok,
            2 => Authentication::KerberosV5,
            3 => Authentication::CleartextPassword,
            5 => Authentication::MD5Password { salt: body.get_u32(), },
            7 => Authentication::GSS,
            9 => Authentication::SSPI,
            10 => Authentication::SASL,
            auth => return Err(ProtocolError::unknown_auth(auth)),
        };
        Ok(auth)
    }
}

/// Identifies the message as cancellation key data.
///
/// The frontend must save these values if it wishes to be able to issue CancelRequest messages later.
#[derive(Debug)]
pub struct BackendKeyData {
    /// The process ID of this backend.
    pub process_id: i32,
    /// The secret key of this backend.
    pub secret_key: i32,
}

impl BackendKeyData {
    pub const MSGTYPE: u8 = b'K';
}

impl BackendProtocol for BackendKeyData {
    fn decode(msgtype: u8, mut body: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(BackendKeyData,msgtype);
        Ok(Self {
            process_id: body.get_i32(),
            secret_key: body.get_i32(),
        })
    }
}

/// Identifies the message as a run-time parameter status report
#[derive(Debug)]
pub struct ParameterStatus {
    /// The name of the run-time parameter being reported
    pub name: String,
    /// The current value of the parameter
    pub value: String
}

impl ParameterStatus {
    pub const MSGTYPE: u8 = b'S';
}

impl BackendProtocol for ParameterStatus {
    fn decode(msgtype: u8, mut body: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(ParameterStatus,msgtype);
        Ok(Self {
            name: body.get_nul_string()?,
            value: body.get_nul_string()?,
        })
    }
}

/// Identifies the message type. ReadyForQuery is sent whenever the backend is ready for a new query cycle.
#[derive(Debug)]
pub struct ReadyForQuery;

impl ReadyForQuery {
    pub const MSGTYPE: u8 = b'Z';
}

impl BackendProtocol for ReadyForQuery {
    fn decode(msgtype: u8, _: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(ReadyForQuery,msgtype);
        Ok(Self)
    }
}

/// A warning message. The frontend should display the message.
///
/// for detail of the body form, see [`MessageFields`]
pub struct NoticeResponse {
    pub body: Bytes
}

impl NoticeResponse {
    pub const MSGTYPE: u8 = b'N';
}

impl BackendProtocol for NoticeResponse {
    fn decode(msgtype: u8, body: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(NoticeResponse,msgtype);
        Ok(NoticeResponse { body })
    }
}

/// Identifies the message as an error
///
/// The message body consists of one or more identified fields, followed by a zero byte as a terminator.
/// Fields can appear in any order.
///
/// For each field there is the following:
///
/// `Byte1` A code identifying the field type; if zero, this is the message terminator and no string follows.
/// The presently defined field types are listed in Section 53.8.
/// Since more field types might be added in future,
/// frontends should silently ignore fields of unrecognized type.
///
/// `String` The field value.
#[derive(Debug, thiserror::Error)]
#[error("{body:?}")]
pub struct ErrorResponse {
    pub body: Bytes,
}

impl ErrorResponse {
    pub const MSGTYPE: u8 = b'E';

    pub fn to_db_error(self) -> DatabaseError {
        DatabaseError::from_error_response(self.body)
    }
}

impl BackendProtocol for ErrorResponse {
    fn decode(msgtype: u8, body: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(ErrorResponse,msgtype);
        Ok(Self { body })
    }
}

/// Identifies the message as a row description
#[derive(Debug)]
pub struct RowDescription {
    /// Specifies the number of fields in a row (can be zero).
    pub field_len: i16,
    pub field_name: String,
    pub table_oid: i32,
    pub attribute_len: i16,
    pub data_type: i32,
    pub data_type_size: i16,
    pub type_modifier: i32,
    pub format_code: i16,
}

impl RowDescription {
    pub const MSGTYPE: u8 = b'T';
}

impl BackendProtocol for RowDescription {
    fn decode(msgtype: u8, mut body: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(RowDescription,msgtype);
        Ok(Self {
            // Int16 Specifies the number of fields in a row (can be zero).
            field_len: body.get_i16(),
            // Int16 Specifies the number of fields in a row (can be zero).
            field_name: body.get_nul_string()?,
            // If the field can be identified as a column of a specific table,
            // the object ID of the table; otherwise zero
            table_oid: body.get_i32(),
            // If the field can be identified as a column of a specific table,
            // the attribute number of the column; otherwise zero.
            attribute_len: body.get_i16(),
            // The object ID of the field's data type.
            data_type: body.get_i32(),
            // The data type size (see pg_type.typlen).
            // Note that negative values denote variable-width types.
            data_type_size: body.get_i16(),
            // The type modifier (see pg_attribute.atttypmod).
            // The meaning of the modifier is type-specific.
            type_modifier: body.get_i32(),
            // The format code being used for the field.
            // Currently will be zero (text) or one (binary).
            // In a RowDescription returned from the statement variant of Describe,
            // the format code is not yet known and will always be zero.
            format_code: body.get_i16(),
        })
    }
}

#[derive(Debug)]
/// Identifies the message as a data row.
pub struct DataRow {
    pub row_buffer: RowBuffer,
}

impl DataRow {
    pub const MSGTYPE: u8 = b'D';
}

impl BackendProtocol for DataRow {
    fn decode(msgtype: u8, mut body: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(DataRow,msgtype);

        // The number of column values that follow (possibly zero).
        let col_values_len = body.get_i16();

        // lazily decode row without allocating `Vec`
        let row_buffer = RowBuffer::new(col_values_len, body);

        Ok(Self { row_buffer })
    }
}

/// Identifies the message as a command-completed response
///
/// For an INSERT command, the tag is INSERT oid rows, where rows is the number of rows inserted.
/// oid used to be the object ID of the inserted row if rows was 1 and the target table had OIDs,
/// but OIDs system columns are not supported anymore; therefore oid is always 0.
///
/// For a DELETE command, the tag is DELETE rows where rows is the number of rows deleted.
///
/// For an UPDATE command, the tag is UPDATE rows where rows is the number of rows updated.
///
/// For a MERGE command, the tag is MERGE rows where rows is the number of rows inserted, updated, or deleted.
///
/// For a SELECT or CREATE TABLE AS command, the tag is SELECT rows where rows is the number of rows retrieved.
///
/// For a MOVE command, the tag is MOVE rows where rows is the number of rows
/// the cursor's position has been changed by.
///
/// For a FETCH command, the tag is FETCH rows where rows is the number of rows that have
/// been retrieved from the cursor.
///
/// For a COPY command, the tag is COPY rows where rows is the number of rows copied.
/// (Note: the row count appears only in PostgreSQL 8.2 and later.)
#[derive(Debug)]
pub struct CommandComplete {
    /// The command tag. This is usually a single word that identifies which SQL command was completed.
    pub tag: String,
}

impl CommandComplete {
    pub const MSGTYPE: u8 = b'C';
}

impl BackendProtocol for CommandComplete {
    fn decode(msgtype: u8, body: Bytes) -> Result<Self, ProtocolError> {
        assert_msgtype!(CommandComplete, msgtype);
        Ok(Self {
            tag: String::from_utf8(body.into()).map_err(ProtocolError::non_utf8)?,
        })
    }
}

/// Identifies the message as a Parse-complete indicator.
#[derive(Debug)]
pub struct ParseComplete;

impl ParseComplete {
    pub const MSGTYPE: u8 = b'1';
}

impl BackendProtocol for ParseComplete {
    fn decode(msgtype: u8, _: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(ParseComplete,msgtype);
        Ok(Self)
    }
}


/// Identifies the message as a Bind-complete indicator.
#[derive(Debug)]
pub struct BindComplete;

impl BindComplete {
    pub const MSGTYPE: u8 = b'2';
}

impl BackendProtocol for BindComplete {
    fn decode(msgtype: u8, _: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(BindComplete,msgtype);
        Ok(Self)
    }
}

/// Identifies the message as a Close-complete indicator.
#[derive(Debug)]
pub struct CloseComplete;

impl CloseComplete {
    pub const MSGTYPE: u8 = b'3';
}

impl BackendProtocol for CloseComplete {
    fn decode(msgtype: u8, _: Bytes) -> Result<Self,ProtocolError> {
        assert_msgtype!(CloseComplete,msgtype);
        Ok(Self)
    }
}

