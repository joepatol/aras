use tokio::io::ErrorKind;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Result as IoResult};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

pub struct LinesCodec {
    reader: BufReader<OwnedReadHalf>,
    writer: BufWriter<OwnedWriteHalf>,
}

impl LinesCodec {
    pub fn new(stream: TcpStream) -> Self {
        let (reader_half, writer_half) = stream.into_split();

        let writer = BufWriter::new(writer_half);
        let reader = BufReader::new(reader_half);

        Self { reader, writer }
    }

    pub async fn send_message(&mut self, message: &[u8]) -> IoResult<()> {
        self.writer.write(message).await?;
        self.writer.write(&['\n' as u8]).await?;
        self.writer.flush().await?;
        Ok(())
    }

    pub async fn read_message(&mut self) -> IoResult<String> {
        let mut received = String::new();
        self.reader.read_line(&mut received).await?;

        // Remove CRLF
        received.pop().ok_or(ErrorKind::InvalidInput)?;
        received.pop().ok_or(ErrorKind::InvalidInput)?;

        Ok(received)
    }
}
