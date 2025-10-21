use anyhow::Result;
use std::net::{UdpSocket, SocketAddr};
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{debug, info, warn};
use crate::database::Database;

const TFTP_PORT: u16 = 69;
const TFTP_ROOT: &str = "/var/lib/casvps/tftp";

#[derive(Debug, Clone)]
pub struct TFTPServer {
    database: Arc<Database>,
    socket: Option<UdpSocket>,
    root_path: PathBuf,
}

#[derive(Debug)]
enum TFTPPacket {
    ReadRequest {
        filename: String,
        mode: String,
    },
    WriteRequest {
        filename: String,
        mode: String,
    },
    Data {
        block: u16,
        data: Vec<u8>,
    },
    Ack {
        block: u16,
    },
    Error {
        code: u16,
        message: String,
    },
}

#[derive(Debug)]
enum TFTPError {
    NotDefined = 0,
    FileNotFound = 1,
    AccessViolation = 2,
    DiskFull = 3,
    IllegalOperation = 4,
    UnknownTransfer = 5,
    FileExists = 6,
    NoSuchUser = 7,
}

impl TFTPServer {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            socket: None,
            root_path: PathBuf::from(TFTP_ROOT),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting TFTP server on port {}", TFTP_PORT);

        // Create TFTP root directory
        fs::create_dir_all(&self.root_path)?;

        // Set up PXE boot files
        self.setup_pxe_files().await?;

        // Bind to TFTP port
        let addr = SocketAddr::from(([0, 0, 0, 0], TFTP_PORT));
        let socket = UdpSocket::bind(addr)?;
        self.socket = Some(socket);

        // Start server loop
        self.serve().await?;

        Ok(())
    }

    async fn serve(&mut self) -> Result<()> {
        let socket = self.socket.as_ref().unwrap();
        let mut buf = [0u8; 1024];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    if let Ok(packet) = self.parse_tftp_packet(&buf[..len]) {
                        self.handle_tftp_packet(packet, src).await?;
                    }
                }
                Err(e) => {
                    debug!("TFTP server socket error: {}", e);
                }
            }
        }
    }

    async fn handle_tftp_packet(&self, packet: TFTPPacket, src: SocketAddr) -> Result<()> {
        match packet {
            TFTPPacket::ReadRequest { filename, mode } => {
                debug!("TFTP RRQ: {} from {}", filename, src);
                self.handle_read_request(filename, mode, src).await?;
            }
            TFTPPacket::WriteRequest { filename, mode } => {
                debug!("TFTP WRQ: {} from {}", filename, src);
                self.send_error(TFTPError::AccessViolation, "Write not allowed", src).await?;
            }
            _ => {
                debug!("Unexpected TFTP packet type from {}", src);
            }
        }

        Ok(())
    }

    async fn handle_read_request(&self, filename: String, _mode: String, src: SocketAddr) -> Result<()> {
        let file_path = self.root_path.join(&filename);

        // Security check - ensure file is within TFTP root
        if !file_path.starts_with(&self.root_path) {
            self.send_error(TFTPError::AccessViolation, "Access denied", src).await?;
            return Ok(());
        }

        // Check if file exists
        if !file_path.exists() {
            // Try to generate dynamic content
            if let Ok(content) = self.generate_dynamic_content(&filename).await {
                self.send_file_content(content, src).await?;
            } else {
                self.send_error(TFTPError::FileNotFound, "File not found", src).await?;
            }
            return Ok(());
        }

        // Read file
        match fs::read(&file_path) {
            Ok(content) => {
                info!("TFTP serving file: {} ({} bytes) to {}", filename, content.len(), src);
                self.send_file_content(content, src).await?;
            }
            Err(_) => {
                self.send_error(TFTPError::AccessViolation, "Cannot read file", src).await?;
            }
        }

        Ok(())
    }

    async fn send_file_content(&self, content: Vec<u8>, dest: SocketAddr) -> Result<()> {
        // Create new socket for transfer
        let transfer_socket = UdpSocket::bind("0.0.0.0:0")?;
        let block_size = 512;
        let mut block_num = 1u16;

        for chunk in content.chunks(block_size) {
            let data_packet = TFTPPacket::Data {
                block: block_num,
                data: chunk.to_vec(),
            };

            let packet_data = self.serialize_tftp_packet(data_packet)?;
            transfer_socket.send_to(&packet_data, dest)?;

            // Wait for ACK (simplified - should handle timeouts)
            let mut ack_buf = [0u8; 512];
            if let Ok((len, _)) = transfer_socket.recv_from(&mut ack_buf) {
                if let Ok(TFTPPacket::Ack { block }) = self.parse_tftp_packet(&ack_buf[..len]) {
                    if block != block_num {
                        warn!("TFTP ACK block mismatch: expected {}, got {}", block_num, block);
                        break;
                    }
                }
            }

            block_num = block_num.wrapping_add(1);

            // Last packet (less than 512 bytes)
            if chunk.len() < block_size {
                break;
            }
        }

        Ok(())
    }

    async fn send_error(&self, error: TFTPError, message: &str, dest: SocketAddr) -> Result<()> {
        let error_packet = TFTPPacket::Error {
            code: error as u16,
            message: message.to_string(),
        };

        let packet_data = self.serialize_tftp_packet(error_packet)?;

        if let Some(socket) = &self.socket {
            socket.send_to(&packet_data, dest)?;
        }

        Ok(())
    }

    async fn setup_pxe_files(&self) -> Result<()> {
        info!("Setting up PXE boot files");

        // Create pxelinux.cfg directory
        let pxe_cfg_dir = self.root_path.join("pxelinux.cfg");
        fs::create_dir_all(&pxe_cfg_dir)?;

        // Create default PXE menu
        let default_menu = self.generate_pxe_menu().await?;
        fs::write(pxe_cfg_dir.join("default"), default_menu)?;

        // Create menu.c32 (simple boot menu)
        self.create_menu_files().await?;

        Ok(())
    }

    async fn generate_pxe_menu(&self) -> Result<String> {
        let menu = r#"DEFAULT menu.c32
PROMPT 0
TIMEOUT 300
ONTIMEOUT local

MENU TITLE CasVPS PXE Boot Menu
MENU BACKGROUND casvps-bg.png

LABEL local
    MENU LABEL Boot from local disk
    MENU DEFAULT
    LOCALBOOT 0

LABEL memtest
    MENU LABEL Memory Test
    KERNEL memtest86+.bin

MENU SEPARATOR

LABEL debian12
    MENU LABEL Debian 12 (Bookworm)
    KERNEL debian/vmlinuz
    APPEND initrd=debian/initrd.gz url=http://172.16.1.1:8006/preseed/debian12.cfg

LABEL ubuntu22
    MENU LABEL Ubuntu 22.04 LTS
    KERNEL ubuntu/vmlinuz
    APPEND initrd=ubuntu/initrd.gz url=http://172.16.1.1:8006/preseed/ubuntu22.cfg

LABEL almalinux9
    MENU LABEL AlmaLinux 9
    KERNEL almalinux/vmlinuz
    APPEND initrd=almalinux/initrd.img inst.repo=http://172.16.1.1:8006/repo/almalinux9

MENU SEPARATOR

LABEL rescue
    MENU LABEL CasVPS Rescue System
    KERNEL rescue/vmlinuz
    APPEND initrd=rescue/initrd.gz boot=live

LABEL diagnostics
    MENU LABEL Hardware Diagnostics
    KERNEL diagnostics/vmlinuz
    APPEND initrd=diagnostics/initrd.gz

"#;

        Ok(menu.to_string())
    }

    async fn create_menu_files(&self) -> Result<()> {
        // Create simple menu files (in a real implementation, these would be actual binaries)
        let menu_c32 = b"menu.c32 placeholder";
        fs::write(self.root_path.join("menu.c32"), menu_c32)?;

        let memtest = b"memtest86+ placeholder";
        fs::write(self.root_path.join("memtest86+.bin"), memtest)?;

        Ok(())
    }

    async fn generate_dynamic_content(&self, filename: &str) -> Result<Vec<u8>> {
        match filename {
            "pxelinux.cfg/default" => {
                let menu = self.generate_pxe_menu().await?;
                Ok(menu.into_bytes())
            }
            path if path.starts_with("pxelinux.cfg/01-") => {
                // MAC-based configuration
                let mac = &path[17..]; // Remove "pxelinux.cfg/01-" prefix
                let config = self.generate_mac_specific_config(mac).await?;
                Ok(config.into_bytes())
            }
            _ => Err(anyhow::anyhow!("No dynamic content for {}", filename))
        }
    }

    async fn generate_mac_specific_config(&self, mac: &str) -> Result<String> {
        // Check database for MAC-specific boot configuration
        debug!("Generating MAC-specific PXE config for {}", mac);

        // Default configuration
        Ok(r#"DEFAULT local
PROMPT 0
TIMEOUT 50
LABEL local
    LOCALBOOT 0
"#.to_string())
    }

    fn parse_tftp_packet(&self, data: &[u8]) -> Result<TFTPPacket> {
        if data.len() < 2 {
            return Err(anyhow::anyhow!("TFTP packet too small"));
        }

        let opcode = u16::from_be_bytes([data[0], data[1]]);

        match opcode {
            1 => {
                // Read Request
                let (filename, mode) = self.parse_request_packet(&data[2..])?;
                Ok(TFTPPacket::ReadRequest { filename, mode })
            }
            2 => {
                // Write Request
                let (filename, mode) = self.parse_request_packet(&data[2..])?;
                Ok(TFTPPacket::WriteRequest { filename, mode })
            }
            3 => {
                // Data
                if data.len() < 4 {
                    return Err(anyhow::anyhow!("Invalid data packet"));
                }
                let block = u16::from_be_bytes([data[2], data[3]]);
                let data_vec = data[4..].to_vec();
                Ok(TFTPPacket::Data { block, data: data_vec })
            }
            4 => {
                // ACK
                if data.len() < 4 {
                    return Err(anyhow::anyhow!("Invalid ACK packet"));
                }
                let block = u16::from_be_bytes([data[2], data[3]]);
                Ok(TFTPPacket::Ack { block })
            }
            5 => {
                // Error
                if data.len() < 4 {
                    return Err(anyhow::anyhow!("Invalid error packet"));
                }
                let code = u16::from_be_bytes([data[2], data[3]]);
                let message = String::from_utf8_lossy(&data[4..data.len()-1]).to_string();
                Ok(TFTPPacket::Error { code, message })
            }
            _ => Err(anyhow::anyhow!("Unknown TFTP opcode: {}", opcode))
        }
    }

    fn parse_request_packet(&self, data: &[u8]) -> Result<(String, String)> {
        let mut parts = Vec::new();
        let mut start = 0;

        for (i, &byte) in data.iter().enumerate() {
            if byte == 0 {
                if start < i {
                    parts.push(String::from_utf8_lossy(&data[start..i]).to_string());
                }
                start = i + 1;
            }
        }

        if parts.len() >= 2 {
            Ok((parts[0].clone(), parts[1].clone()))
        } else {
            Err(anyhow::anyhow!("Invalid request packet format"))
        }
    }

    fn serialize_tftp_packet(&self, packet: TFTPPacket) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        match packet {
            TFTPPacket::Data { block, data: file_data } => {
                data.extend_from_slice(&3u16.to_be_bytes()); // Opcode
                data.extend_from_slice(&block.to_be_bytes());
                data.extend_from_slice(&file_data);
            }
            TFTPPacket::Ack { block } => {
                data.extend_from_slice(&4u16.to_be_bytes()); // Opcode
                data.extend_from_slice(&block.to_be_bytes());
            }
            TFTPPacket::Error { code, message } => {
                data.extend_from_slice(&5u16.to_be_bytes()); // Opcode
                data.extend_from_slice(&code.to_be_bytes());
                data.extend_from_slice(message.as_bytes());
                data.push(0); // Null terminator
            }
            _ => {
                return Err(anyhow::anyhow!("Cannot serialize this packet type"));
            }
        }

        Ok(data)
    }
}