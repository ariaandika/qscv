use bytes::{Buf, Bytes};

/// Decoded error from [`ErrorResponse`] body
///
/// Each field type has a single-byte identification token.
///
/// Note that any given field type should appear at most once per message.
///
/// <https://www.postgresql.org/docs/current/protocol-error-fields.html>
///
/// [`ErrorResponse`]: crate::message::backend::ErrorResponse
#[derive(Debug)]
pub struct DatabaseError {
    /// one of [`Severity`], or a localized translation of one of these, always present
    ///
    /// id token: `b'S'`
    pub severity_localized: String,
    /// this is identical to the S field except that the contents are never localized.
    ///
    /// this is present only in messages generated by PostgreSQL versions 9.6 and later.
    ///
    /// id token: `b'V'`
    pub severity: Option<String>,
    /// the SQLSTATE code for the error. Not localizable. Always present.
    ///
    /// see [Appendix A](https://www.postgresql.org/docs/current/errcodes-appendix.html)
    ///
    /// id token: `b'C'`
    pub code: String,
    /// the primary human-readable error message. Always present.
    ///
    /// This should be accurate but terse (typically one line).
    ///
    /// id token: `b'M'`
    pub message: String,
    /// an optional secondary error message carrying more detail about the problem.
    ///
    /// Might run to multiple lines.
    ///
    /// id token: `b'D'`
    pub detail: Option<String>,
    /// an optional suggestion what to do about the problem.
    ///
    /// This is intended to differ from Detail in that it offers advice (potentially inappropriate)
    /// rather than hard facts.
    ///
    /// Might run to multiple lines.
    ///
    /// id token: `b'H'`
    pub hint: Option<String>,
    /// the field value is a decimal ASCII integer, indicating an error cursor position as an index into
    /// the original query string.
    ///
    /// The first character has index 1, and positions are measured in characters not bytes.
    ///
    /// id token: `b'P'`
    pub position: Option<String>,
    /// this is defined the same as the P field, but it is used when the cursor position refers to an internally
    /// generated command rather than the one submitted by the client.
    ///
    /// The q field will always appear when this field appears.
    ///
    /// id token: `b'p'`
    pub internal_position: Option<String>,
    /// the text of a failed internally-generated command. This could be, for example, an SQL query
    /// issued by a PL/pgSQL function.
    ///
    /// id token: `b'q'`
    pub internal_query: Option<String>,
    /// an indication of the context in which the error occurred.
    ///
    /// Presently this includes a call stack traceback of active procedural language functions and
    /// internally-generated queries. The trace is one entry per line, most recent first.
    ///
    /// id token: `b'W'`
    pub where_: Option<String>,
    /// if the error was associated with a specific database object, the name of the schema containing that object,
    /// if any.
    ///
    /// id token: `b's'`
    pub schema_name: Option<String>,
    /// if the error was associated with a specific table, the name of the table.
    /// (Refer to the schema name field for the name of the table's schema.)
    ///
    /// id token: `b't'`
    pub table_name: Option<String>,
    /// if the error was associated with a specific table column, the name of the column.
    /// (Refer to the schema and table name fields to identify the table.)
    ///
    /// id token: `b'c'`
    pub column_name: Option<String>,
    /// if the error was associated with a specific data type, the name of the data type.
    /// (Refer to the schema name field for the name of the data type's schema.)
    ///
    /// id token: `b'd'`
    pub data_type_name: Option<String>,
    /// if the error was associated with a specific constraint, the name of the constraint.
    ///
    /// Refer to fields listed above for the associated table or domain.
    /// (For this purpose, indexes are treated as constraints, even if they weren't created with constraint syntax.)
    ///
    /// id token: `b'n'`
    pub constraint_name: Option<String>,
    /// the file name of the source-code location where the error was reported.
    ///
    /// id token: `b'F'`
    pub file_name: Option<String>,
    /// the line number of the source-code location where the error was reported.
    ///
    /// id token: `b'L'`
    pub line: Option<String>,
    /// the name of the source-code routine reporting the error.
    ///
    /// id token: `b'R'`
    pub routine: Option<String>,
}

macro_rules! nul_string {
    ($b:ident) => {
        match $b.iter().position(|e|matches!(e,b'\0')) {
            Some(end) => {
                let b2 = $b.split_to(end);
                match std::str::from_utf8(&b2[..]) {
                    Ok(ok) => ok.to_owned(),
                    Err(_) => format!("{b2:?}"),
                }
            },
            None => format!("<non nul string in error response>")
        }
    };
}

impl DatabaseError {
    /// Decode from [`ErrorResponse`] body
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
    ///
    /// [`ErrorResponse`]: crate::message::backend::ErrorResponse
    pub fn from_error_response(mut bytes: Bytes) -> DatabaseError {
        let mut me = Self::private_default();

        // NOTE: all error is gracefully handled
        // were in error handling area already

        loop {
            let f = bytes.get_u8();
            if f == b'\0' {
                break
            }
            match f {
                b'S' => { me.severity_localized = nul_string!(bytes); }
                b'V' => { me.severity.replace(nul_string!(bytes)); },
                b'C' => { me.code = nul_string!(bytes); }
                b'M' => { me.message = nul_string!(bytes); }
                b'D' => { me.detail.replace(nul_string!(bytes)); }
                b'H' => { me.hint.replace(nul_string!(bytes)); }
                b'P' => { me.position.replace(nul_string!(bytes)); }
                b'p' => { me.internal_position.replace(nul_string!(bytes)); }
                b'q' => { me.internal_query.replace(nul_string!(bytes)); }
                b'W' => { me.where_.replace(nul_string!(bytes)); }
                b's' => { me.schema_name.replace(nul_string!(bytes)); }
                b't' => { me.table_name.replace(nul_string!(bytes)); }
                b'c' => { me.column_name.replace(nul_string!(bytes)); }
                b'd' => { me.data_type_name.replace(nul_string!(bytes)); }
                b'n' => { me.constraint_name.replace(nul_string!(bytes)); }
                b'F' => { me.file_name.replace(nul_string!(bytes)); }
                b'L' => { me.line.replace(nul_string!(bytes)); }
                b'R' => { me.routine.replace(nul_string!(bytes)); }
                _ => {},
            }
        }

        me
    }
}


impl DatabaseError {
    fn private_default() -> Self {
        Self {
            severity_localized: String::from("severity field missing"),
            severity: Default::default(),
            code: String::from("code field missing"),
            message: String::from("message field missing"),
            detail: Default::default(),
            hint: Default::default(),
            position: Default::default(),
            internal_position: Default::default(),
            internal_query: Default::default(),
            where_: Default::default(),
            schema_name: Default::default(),
            table_name: Default::default(),
            column_name: Default::default(),
            data_type_name: Default::default(),
            constraint_name: Default::default(),
            file_name: Default::default(),
            line: Default::default(),
            routine: Default::default(),
        }
    }
}

impl std::error::Error for DatabaseError { }

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} ({})",self.severity_localized,self.message,self.code)?;
        match &self.hint {
            Some(ok) => write!(f, ", HINT: {ok}")?,
            None => {},
        }
        Ok(())
    }
}

// TODO: Appendix A, error code / sqlstate message
// https://www.postgresql.org/docs/current/errcodes-appendix.html

