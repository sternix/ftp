use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Represents an FTP response with a status code and message.
#[derive(Debug)]
pub struct FtpResponse {
    pub code: u32,
    pub message: String,
}

impl FtpResponse {
    pub fn is_success(&self) -> bool {
        self.code >= 100 && self.code < 400
    }
}

/// FTP transfer mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferMode {
    Active,
    Passive,
}

/// FTP transfer type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferType {
    Ascii,
    Binary,
}

/// A simple FTP client.
pub struct FtpClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    pub transfer_mode: TransferMode,
    pub transfer_type: TransferType,
}

impl FtpClient {
    /// Connect to an FTP server at the given address.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<(Self, FtpResponse)> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;

        let writer = stream.try_clone()?;
        let reader = BufReader::new(stream);

        let mut client = FtpClient {
            reader,
            writer,
            transfer_mode: TransferMode::Passive,
            transfer_type: TransferType::Binary,
        };

        let response = client.read_response()?;
        Ok((client, response))
    }

    /// Send a raw FTP command and read the response.
    pub fn send_command(&mut self, cmd: &str) -> io::Result<FtpResponse> {
        write!(self.writer, "{}\r\n", cmd)?;
        self.writer.flush()?;
        self.read_response()
    }

    /// Read a response from the server, handling multi-line responses.
    fn read_response(&mut self) -> io::Result<FtpResponse> {
        let mut full_message = String::new();
        let mut final_code: Option<u32> = None;

        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line)?;

            if line.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Connection closed by server",
                ));
            }

            let trimmed = line.trim_end();
            full_message.push_str(trimmed);
            full_message.push('\n');

            // Check if this is the final line of the response.
            // Final lines have format: "XXX " (3-digit code followed by space).
            // Continuation lines have: "XXX-" (3-digit code followed by dash).
            if trimmed.len() >= 4 {
                if let Ok(code) = trimmed[..3].parse::<u32>() {
                    let separator = trimmed.as_bytes()[3];
                    if separator == b' ' {
                        final_code = Some(code);
                        break;
                    }
                }
            }
        }

        let code = final_code.unwrap_or(0);
        Ok(FtpResponse {
            code,
            message: full_message.trim().to_string(),
        })
    }

    /// Login with username and password.
    pub fn login(&mut self, user: &str, pass: &str) -> io::Result<FtpResponse> {
        let resp = self.send_command(&format!("USER {}", user))?;
        // 331 means password required
        if resp.code == 331 {
            self.send_command(&format!("PASS {}", pass))
        } else {
            Ok(resp)
        }
    }

    /// Enter passive mode and return the data connection.
    fn open_passive_data_connection(&mut self) -> io::Result<TcpStream> {
        let resp = self.send_command("PASV")?;
        if resp.code != 227 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("PASV failed: {}", resp.message),
            ));
        }

        // Parse the PASV response to extract the address.
        // Format: 227 Entering Passive Mode (h1,h2,h3,h4,p1,p2)
        let start = resp
            .message
            .find('(')
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Invalid PASV response"))?;
        let end = resp
            .message
            .find(')')
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Invalid PASV response"))?;
        let nums: Vec<u32> = resp.message[start + 1..end]
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if nums.len() != 6 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Invalid PASV address format",
            ));
        }

        let addr = format!(
            "{}.{}.{}.{}:{}",
            nums[0],
            nums[1],
            nums[2],
            nums[3],
            nums[4] * 256 + nums[5]
        );

        let stream = TcpStream::connect(&addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        Ok(stream)
    }

    /// Set the transfer type (ASCII or Binary).
    pub fn set_type(&mut self, transfer_type: TransferType) -> io::Result<FtpResponse> {
        let cmd = match transfer_type {
            TransferType::Ascii => "TYPE A",
            TransferType::Binary => "TYPE I",
        };
        let resp = self.send_command(cmd)?;
        if resp.is_success() {
            self.transfer_type = transfer_type;
        }
        Ok(resp)
    }

    /// Print working directory.
    pub fn pwd(&mut self) -> io::Result<FtpResponse> {
        self.send_command("PWD")
    }

    /// Change working directory.
    pub fn cwd(&mut self, path: &str) -> io::Result<FtpResponse> {
        self.send_command(&format!("CWD {}", path))
    }

    /// Change to parent directory.
    pub fn cdup(&mut self) -> io::Result<FtpResponse> {
        self.send_command("CDUP")
    }

    /// List files in the current (or given) directory.
    pub fn list(&mut self, path: Option<&str>) -> io::Result<(FtpResponse, String)> {
        let data_stream = self.open_passive_data_connection()?;

        let cmd = match path {
            Some(p) => format!("LIST {}", p),
            None => "LIST".to_string(),
        };
        let resp = self.send_command(&cmd)?;
        if !resp.is_success() {
            return Ok((resp, String::new()));
        }

        let mut data = String::new();
        let mut reader = BufReader::new(data_stream);
        reader.read_to_string(&mut data)?;

        let final_resp = self.read_response()?;
        Ok((final_resp, data))
    }

    /// Get file/directory names only (NLST).
    pub fn nlst(&mut self, path: Option<&str>) -> io::Result<(FtpResponse, String)> {
        let data_stream = self.open_passive_data_connection()?;

        let cmd = match path {
            Some(p) => format!("NLST {}", p),
            None => "NLST".to_string(),
        };
        let resp = self.send_command(&cmd)?;
        if !resp.is_success() {
            return Ok((resp, String::new()));
        }

        let mut data = String::new();
        let mut reader = BufReader::new(data_stream);
        reader.read_to_string(&mut data)?;

        let final_resp = self.read_response()?;
        Ok((final_resp, data))
    }

    /// Download a file from the server.
    pub fn download(&mut self, remote_path: &str) -> io::Result<(FtpResponse, Vec<u8>)> {
        let data_stream = self.open_passive_data_connection()?;

        let resp = self.send_command(&format!("RETR {}", remote_path))?;
        if !resp.is_success() {
            return Ok((resp, Vec::new()));
        }

        let mut data = Vec::new();
        let mut reader = BufReader::new(data_stream);
        reader.read_to_end(&mut data)?;

        let final_resp = self.read_response()?;
        Ok((final_resp, data))
    }

    /// Upload a file to the server.
    pub fn upload(&mut self, remote_path: &str, data: &[u8]) -> io::Result<FtpResponse> {
        let mut data_stream = self.open_passive_data_connection()?;

        let resp = self.send_command(&format!("STOR {}", remote_path))?;
        if !resp.is_success() {
            return Ok(resp);
        }

        data_stream.write_all(data)?;
        data_stream.flush()?;
        drop(data_stream);

        self.read_response()
    }

    /// Append data to a file on the server.
    pub fn append(&mut self, remote_path: &str, data: &[u8]) -> io::Result<FtpResponse> {
        let mut data_stream = self.open_passive_data_connection()?;

        let resp = self.send_command(&format!("APPE {}", remote_path))?;
        if !resp.is_success() {
            return Ok(resp);
        }

        data_stream.write_all(data)?;
        data_stream.flush()?;
        drop(data_stream);

        self.read_response()
    }

    /// Delete a file on the server.
    pub fn delete(&mut self, path: &str) -> io::Result<FtpResponse> {
        self.send_command(&format!("DELE {}", path))
    }

    /// Create a directory on the server.
    pub fn mkdir(&mut self, path: &str) -> io::Result<FtpResponse> {
        self.send_command(&format!("MKD {}", path))
    }

    /// Remove a directory on the server.
    pub fn rmdir(&mut self, path: &str) -> io::Result<FtpResponse> {
        self.send_command(&format!("RMD {}", path))
    }

    /// Rename a file or directory.
    pub fn rename(&mut self, from: &str, to: &str) -> io::Result<FtpResponse> {
        let resp = self.send_command(&format!("RNFR {}", from))?;
        if resp.code != 350 {
            return Ok(resp);
        }
        self.send_command(&format!("RNTO {}", to))
    }

    /// Get the size of a file.
    pub fn size(&mut self, path: &str) -> io::Result<FtpResponse> {
        self.send_command(&format!("SIZE {}", path))
    }

    /// Get the modification time of a file.
    pub fn mdtm(&mut self, path: &str) -> io::Result<FtpResponse> {
        self.send_command(&format!("MDTM {}", path))
    }

    /// Get system type.
    pub fn syst(&mut self) -> io::Result<FtpResponse> {
        self.send_command("SYST")
    }

    /// Get server status.
    pub fn stat(&mut self, path: Option<&str>) -> io::Result<FtpResponse> {
        match path {
            Some(p) => self.send_command(&format!("STAT {}", p)),
            None => self.send_command("STAT"),
        }
    }

    /// No-op (keep alive).
    pub fn noop(&mut self) -> io::Result<FtpResponse> {
        self.send_command("NOOP")
    }

    /// Quit the FTP session.
    pub fn quit(&mut self) -> io::Result<FtpResponse> {
        self.send_command("QUIT")
    }
}
