use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use crate::protocol::{Request, Response, SOCKET_PATH};

pub struct Client {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Client {
    pub fn new() -> io::Result<Self> {
        let stream = UnixStream::connect(SOCKET_PATH)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self { stream, reader })
    }

    pub fn send(&mut self, req: Request) -> io::Result<Response> {
        let mut data = serde_json::to_string(&req)?;
        data.push('\n');
        self.stream.write_all(data.as_bytes())?;
        self.stream.flush()?;

        let mut buf = String::new();
        self.reader.read_line(&mut buf)?;
        
        let resp: Response = serde_json::from_str(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            
        Ok(resp)
    }
}
