use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter, Result as IoResult};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use crate::error::Error;

pub struct LinesCodec {
    pub reader: BufReader<OwnedReadHalf>,
    writer: BufWriter<OwnedWriteHalf>,
}

impl LinesCodec {
    pub fn new(stream: TcpStream) -> Self {
        let (reader_half, writer_half) = stream.into_split();

        let writer = BufWriter::new(writer_half);
        let reader = BufReader::new(reader_half);

        Self { reader, writer }
    }

    pub async fn send_message(&mut self, message: String) -> IoResult<()> {
        self.writer.write(message.as_bytes()).await?;
        self.writer.write(&['\n' as u8]).await?;
        self.writer.flush().await?;
        Ok(())
    }

    pub async fn read_message(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        Ok(self.reader.read(buffer).await?)
    }
}

impl From<TcpStream> for LinesCodec {
    fn from(value: TcpStream) -> Self {
        LinesCodec::new(value)
    }
}

impl TryFrom<LinesCodec> for TcpStream {
    type Error = Error;

    fn try_from(value: LinesCodec) -> std::prelude::v1::Result<Self, Self::Error> {
        Ok(
            value.reader.into_inner().reunite(value.writer.into_inner())
            .map_err(|e| Error::custom(e.to_string()))?
        )
    }
}