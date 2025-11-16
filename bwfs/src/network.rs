use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Network request types
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    ReadBlock { block_num: u32 },
    WriteBlock { block_num: u32, data: Vec<u8> },
    Ping,
}

/// Network response types
#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    BlockData { data: Vec<u8> },
    Success,
    Error { message: String },
    Pong,
}

/// Network server for distributed BWFS
pub struct NetworkServer {
    port: u16,
}

impl NetworkServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
    
    /// Start the network server
    pub async fn start(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        
        log::info!("BWFS network server listening on {}", addr);
        
        loop {
            let (socket, addr) = listener.accept().await?;
            log::debug!("New connection from {}", addr);
            
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket).await {
                    log::error!("Connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(mut socket: TcpStream) -> Result<()> {
    let mut buf = vec![0u8; 8192];
    
    loop {
        let n = socket.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        
        let request: Request = serde_json::from_slice(&buf[..n])?;
        let response = process_request(request).await;
        
        let response_data = serde_json::to_vec(&response)?;
        socket.write_all(&response_data).await?;
    }
    
    Ok(())
}

async fn process_request(request: Request) -> Response {
    match request {
        Request::Ping => Response::Pong,
        Request::ReadBlock { block_num: _ } => {
            // TODO: Implement actual block reading
            Response::BlockData { data: vec![0; 1024] }
        }
        Request::WriteBlock { block_num: _, data: _ } => {
            // TODO: Implement actual block writing
            Response::Success
        }
    }
}

/// Network client for accessing remote blocks
pub struct NetworkClient {
    nodes: Vec<String>,
}

impl NetworkClient {
    pub fn new(nodes: Vec<String>) -> Self {
        Self { nodes }
    }
    
    /// Read a block from a remote node
    pub async fn read_block(&self, node_idx: usize, block_num: u32) -> Result<Vec<u8>> {
        if node_idx >= self.nodes.len() {
            anyhow::bail!("Invalid node index");
        }
        
        let addr = &self.nodes[node_idx];
        let mut stream = TcpStream::connect(addr).await?;
        
        let request = Request::ReadBlock { block_num };
        let request_data = serde_json::to_vec(&request)?;
        
        stream.write_all(&request_data).await?;
        
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await?;
        
        let response: Response = serde_json::from_slice(&buf[..n])?;
        
        match response {
            Response::BlockData { data } => Ok(data),
            Response::Error { message } => anyhow::bail!(message),
            _ => anyhow::bail!("Unexpected response"),
        }
    }
    
    /// Write a block to a remote node
    pub async fn write_block(&self, node_idx: usize, block_num: u32, data: Vec<u8>) -> Result<()> {
        if node_idx >= self.nodes.len() {
            anyhow::bail!("Invalid node index");
        }
        
        let addr = &self.nodes[node_idx];
        let mut stream = TcpStream::connect(addr).await?;
        
        let request = Request::WriteBlock { block_num, data };
        let request_data = serde_json::to_vec(&request)?;
        
        stream.write_all(&request_data).await?;
        
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await?;
        
        let response: Response = serde_json::from_slice(&buf[..n])?;
        
        match response {
            Response::Success => Ok(()),
            Response::Error { message } => anyhow::bail!(message),
            _ => anyhow::bail!("Unexpected response"),
        }
    }
}
