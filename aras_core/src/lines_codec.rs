use tokio::io::AsyncReadExt;
use tokio::io::{AsyncWriteExt, BufReader, BufWriter, Result as IoResult};
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
