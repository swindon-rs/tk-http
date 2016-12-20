use tokio_core::io::Io;

pub struct BufferedService {
    request: Option<Request>,
}

impl<S: Io> Codec<S> for BufferedService {
    fn headers_received(&mut self, headers: &Head)
        -> Result<RecvMode, Error>
    {
    }
}
