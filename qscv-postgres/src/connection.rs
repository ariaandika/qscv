use std::num::NonZeroUsize;
use lru::LruCache;

use crate::{
    encode::Encoded,
    error::Result,
    message::{
        error::ProtocolError,
        frontend::{Bind, Execute, Parse, Sync},
        BackendMessage,
    },
    options::PgOptions,
    row_buffer::RowBuffer,
    stream::PgStream,
};

const DEFAULT_PREPARED_STMT_CACHE: NonZeroUsize = NonZeroUsize::new(24).unwrap();

#[derive(Debug)]
pub struct PgConnection {
    stream: PgStream,
    stmt_id: std::num::NonZeroU32,
    portal_id: std::num::NonZeroU32,
    prepared_stmt: LruCache<String, String>,
}

impl PgConnection {
    /// perform a startup message via url
    pub async fn connect(url: &str) -> Result<Self> {
        Self::connect_with(PgOptions::parse(url)?).await
    }

    /// perform a startup message with options
    pub async fn connect_with(opt: PgOptions) -> Result<Self> {
        let mut stream = PgStream::connect(&opt).await?;

        let crate::protocol::StartupResponse {
            backend_key_data: _,
            param_status: _,
        } = crate::protocol::startup(&opt, &mut stream).await?;

        Ok(Self {
            stream,
            stmt_id: std::num::NonZeroU32::new(1).unwrap(),
            portal_id: std::num::NonZeroU32::new(1).unwrap(),
            prepared_stmt: LruCache::new(DEFAULT_PREPARED_STMT_CACHE),
        })
    }

    /// perform an extended query
    ///
    /// <https://www.postgresql.org/docs/current/protocol-flow.html#PROTOCOL-FLOW-EXT-QUERY>
    pub async fn query(&mut self, sql: &str, args: &[Encoded<'_>]) -> Result<Vec<RowBuffer>> {
        if let Some(_cached) = self.prepared_stmt.get_mut(sql) {
            todo!()
        }

        if self.stmt_id.checked_add(1).is_none() {
            self.stmt_id = std::num::NonZeroU32::new(1).unwrap();
        }

        if self.portal_id.checked_add(1).is_none() {
            self.portal_id = std::num::NonZeroU32::new(1).unwrap();
        }

        let mut b = itoa::Buffer::new();
        let mut b2 = itoa::Buffer::new();
        let prepare_name = b.format(self.stmt_id.get());
        let portal_name = b2.format(self.portal_id.get());

        // In the extended protocol, the frontend first sends a Parse message

        // WARN: is this documented somewhere ?
        // Apparantly, sending Parse command, postgres does not immediately
        // response with ParseComplete.
        // 1. sending Sync immediately will do so
        // 2. otherwise, we can continue the protocol without waiting for one

        self.stream.send(Parse {
            prepare_name,
            sql,
            data_types_len: args.len() as _,
            data_types: args.iter().map(Encoded::oid),
        });

        // Once a prepared statement exists, it can be readied for execution using a Bind message.

        self.stream.send(Bind {
            portal_name,
            prepare_name,
            params_format_len: 1,
            params_format_code: [1],
            params_len: args,
            params: args,
            results_format_len: 1,
            results_format_code: [1],
        });

        // Once a portal exists, it can be executed using an Execute message

        self.stream.send(Execute {
            portal_name,
            max_row: 0,
        });

        self.stream.send(Sync);
        self.stream.flush().await?;

        // The response to Parse is either ParseComplete or ErrorResponse
        self.stream.recv::<BackendMessage>().await?;

        // The response to Bind is either BindComplete or ErrorResponse.
        self.stream.recv::<BackendMessage>().await?;

        let mut rows = vec![];

        // The possible responses to Execute are the same as those described above
        // for queries issued via simple query protocol, except that Execute doesn't
        // cause ReadyForQuery or RowDescription to be issued.
        loop {
            use BackendMessage::*;
            match self.stream.recv().await? {
                DataRow(row) => rows.push(row.row_buffer),
                CommandComplete(_) => break,
                f => Err(ProtocolError::unexpected_phase(f.msgtype(), "extended query"))?,
            }
        }

        // The response to Sync is either BindComplete or ErrorResponse.
        self.stream.recv::<BackendMessage>().await?;

        Ok(rows)
    }
}

