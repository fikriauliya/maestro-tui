use std::io::{Read, Write};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

/// Trait for PTY backend operations, enabling dependency injection for testing
#[allow(dead_code)]
pub trait PtyBackend: Send {
    fn write(&mut self, data: &[u8]) -> std::io::Result<()>;
    fn resize(&mut self, cols: u16, rows: u16);
    /// Try to read available data, returns None if no data available
    fn try_read(&mut self) -> std::io::Result<Option<Vec<u8>>>;
}

/// Native PTY implementation using portable-pty
#[allow(dead_code)]
pub struct NativePty {
    writer: Box<dyn Write + Send>,
    reader: Box<dyn Read + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
}

#[allow(dead_code)]
impl NativePty {
    /// Create a new PTY running the user's shell
    pub fn new(cols: u16, rows: u16) -> color_eyre::Result<Self> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        Self::with_command(cols, rows, &shell, &[])
    }

    /// Create a new PTY running a specific command
    pub fn with_command(
        cols: u16,
        rows: u16,
        program: &str,
        args: &[&str],
    ) -> color_eyre::Result<Self> {
        let pty_system = native_pty_system();

        let pty_pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        let mut cmd = CommandBuilder::new(program);
        for arg in args {
            cmd.arg(*arg);
        }
        cmd.cwd(std::env::current_dir()?);

        let _child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        let reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        Ok(Self {
            writer,
            reader,
            master: pty_pair.master,
        })
    }
}

impl PtyBackend for NativePty {
    fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    fn try_read(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        let mut buf = [0u8; 4096];
        match self.reader.read(&mut buf) {
            Ok(0) => Ok(None),
            Ok(n) => Ok(Some(buf[..n].to_vec())),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// Mock PTY for testing - records writes and provides scripted reads
    #[derive(Clone)]
    pub struct MockPty {
        /// All data written to this PTY
        pub written: Arc<Mutex<Vec<Vec<u8>>>>,
        /// Queue of data to return from reads
        pub read_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
        /// Current size
        pub size: Arc<Mutex<(u16, u16)>>,
    }

    impl MockPty {
        pub fn new(cols: u16, rows: u16) -> Self {
            Self {
                written: Arc::new(Mutex::new(Vec::new())),
                read_queue: Arc::new(Mutex::new(VecDeque::new())),
                size: Arc::new(Mutex::new((cols, rows))),
            }
        }

        /// Queue data to be returned by try_read
        pub fn queue_read(&self, data: Vec<u8>) {
            self.read_queue.lock().unwrap().push_back(data);
        }

        /// Get all written data
        pub fn get_written(&self) -> Vec<Vec<u8>> {
            self.written.lock().unwrap().clone()
        }

        /// Get current size
        pub fn get_size(&self) -> (u16, u16) {
            *self.size.lock().unwrap()
        }
    }

    impl PtyBackend for MockPty {
        fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
            self.written.lock().unwrap().push(data.to_vec());
            Ok(())
        }

        fn resize(&mut self, cols: u16, rows: u16) {
            *self.size.lock().unwrap() = (cols, rows);
        }

        fn try_read(&mut self) -> std::io::Result<Option<Vec<u8>>> {
            Ok(self.read_queue.lock().unwrap().pop_front())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockPty;
    use super::*;

    #[test]
    fn test_mock_pty_write() {
        let mut pty = MockPty::new(80, 24);
        pty.write(b"hello").unwrap();
        pty.write(b"world").unwrap();

        let written = pty.get_written();
        assert_eq!(written.len(), 2);
        assert_eq!(written[0], b"hello");
        assert_eq!(written[1], b"world");
    }

    #[test]
    fn test_mock_pty_read() {
        let mut pty = MockPty::new(80, 24);
        pty.queue_read(b"response1".to_vec());
        pty.queue_read(b"response2".to_vec());

        assert_eq!(pty.try_read().unwrap(), Some(b"response1".to_vec()));
        assert_eq!(pty.try_read().unwrap(), Some(b"response2".to_vec()));
        assert_eq!(pty.try_read().unwrap(), None);
    }

    #[test]
    fn test_mock_pty_resize() {
        let mut pty = MockPty::new(80, 24);
        assert_eq!(pty.get_size(), (80, 24));

        pty.resize(120, 40);
        assert_eq!(pty.get_size(), (120, 40));
    }
}
