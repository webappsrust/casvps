use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Serialize, Deserialize)]
pub enum Command {
    Shutdown,
    GetStatus,
    JoinCluster { server: String, token: String },
    LeaveCluster { nodename: String },
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Ok,
    Status(String),
    Error(String),
}

pub struct ServiceClient {
    stream: UnixStream,
}

impl ServiceClient {
    pub async fn connect() -> Result<Self> {
        let stream = UnixStream::connect("/var/run/casvps.sock").await?;
        Ok(Self { stream })
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.send_command(Command::Shutdown).await?;
        Ok(())
    }

    pub async fn get_status(&mut self) -> Result<String> {
        let response = self.send_command(Command::GetStatus).await?;
        match response {
            Response::Status(status) => Ok(status),
            Response::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Unexpected response")),
        }
    }

    pub async fn join_cluster(&mut self, server: &str, token: &str) -> Result<()> {
        self.send_command(Command::JoinCluster {
            server: server.to_string(),
            token: token.to_string(),
        }).await?;
        Ok(())
    }

    pub async fn leave_cluster(&mut self, nodename: &str) -> Result<()> {
        self.send_command(Command::LeaveCluster {
            nodename: nodename.to_string(),
        }).await?;
        Ok(())
    }

    async fn send_command(&mut self, cmd: Command) -> Result<Response> {
        let data = serde_json::to_vec(&cmd)?;
        let len = data.len() as u32;

        // Send length prefix
        self.stream.write_all(&len.to_be_bytes()).await?;
        // Send data
        self.stream.write_all(&data).await?;

        // Read response length
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        // Read response data
        let mut buf = vec![0u8; len];
        self.stream.read_exact(&mut buf).await?;

        let response: Response = serde_json::from_slice(&buf)?;
        Ok(response)
    }
}