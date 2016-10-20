use futures::Future;
use tk_bufstream::IoBuf;
use tokio_core::io::Io;

use {Error, ResponseWriter};


/// A response object
///
/// It's generic because it may be not self-contained and can use any smart
/// techniques to write response:
///
/// * Use `sendfile` syscall
/// * Stream request from source, etc.
///
pub trait GenericResponse<S: Io> {

    /// Actual serializer type
    ///
    /// Should return back TcpSocket and buffer when finished. There is no
    /// obligation that buffer is flushed when finished, we will take care of
    /// flushing
    type Future: Future<Item=IoBuf<S>, Error=Error>;

    /// Create a serializer object
    ///
    /// Buffer that is passed here is not required to be empty. Content of
    /// the buffer must not be discarded when serializing. And serializer is
    /// free to append the data.
    ///
    /// When serializer is going to write to the socket directly it's required
    /// to flush the data from the buffer into the socket first.
    fn into_serializer(self, writer: ResponseWriter<S>)
        -> Self::Future;
}
