use anyhow::Result;
use std::net::{UdpSocket, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};
use crate::database::Database;

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;

#[derive(Debug, Clone)]
pub struct DHCPServer {
    database: Arc<Database>,
    socket: Option<UdpSocket>,
    leases: HashMap<[u8; 6], DHCPLease>,  // MAC -> Lease
    pool_start: Ipv4Addr,
    pool_end: Ipv4Addr,
    gateway: Ipv4Addr,
    dns_servers: Vec<Ipv4Addr>,
    lease_time: u32,
}

#[derive(Debug, Clone)]
struct DHCPLease {
    ip: Ipv4Addr,
    mac: [u8; 6],
    hostname: Option<String>,
    expires: std::time::Instant,
}

#[derive(Debug)]
struct DHCPPacket {
    op: u8,      // 1 = request, 2 = reply
    htype: u8,   // 1 = Ethernet
    hlen: u8,    // 6 for Ethernet
    hops: u8,
    xid: u32,    // Transaction ID
    secs: u16,
    flags: u16,
    ciaddr: Ipv4Addr,  // Client IP
    yiaddr: Ipv4Addr,  // Your IP
    siaddr: Ipv4Addr,  // Server IP
    giaddr: Ipv4Addr,  // Gateway IP
    chaddr: [u8; 16],  // Client hardware address
    sname: [u8; 64],   // Server name
    file: [u8; 128],   // Boot file name
    options: Vec<DHCPOption>,
}

#[derive(Debug, Clone)]
enum DHCPOption {
    MessageType(DHCPMessageType),
    SubnetMask(Ipv4Addr),
    Router(Vec<Ipv4Addr>),
    DomainNameServer(Vec<Ipv4Addr>),
    LeaseTime(u32),
    ServerIdentifier(Ipv4Addr),
    RequestedIP(Ipv4Addr),
    Hostname(String),
    End,
}

#[derive(Debug, Clone, PartialEq)]
enum DHCPMessageType {
    Discover = 1,
    Offer = 2,
    Request = 3,
    Decline = 4,
    Ack = 5,
    Nak = 6,
    Release = 7,
    Inform = 8,
}

impl DHCPServer {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            socket: None,
            leases: HashMap::new(),
            pool_start: "172.16.1.100".parse().unwrap(),
            pool_end: "172.16.1.200".parse().unwrap(),
            gateway: "172.16.1.1".parse().unwrap(),
            dns_servers: vec!["172.16.1.1".parse().unwrap()],
            lease_time: 86400, // 24 hours
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting DHCP server on port {}", DHCP_SERVER_PORT);

        // Bind to DHCP server port
        let addr = SocketAddr::from(([0, 0, 0, 0], DHCP_SERVER_PORT));
        let socket = UdpSocket::bind(addr)?;
        socket.set_broadcast(true)?;

        self.socket = Some(socket);

        // Start lease cleanup task
        let mut cleanup_interval = interval(Duration::from_secs(300)); // 5 minutes

        tokio::spawn(async move {
            loop {
                cleanup_interval.tick().await;
                // TODO: Cleanup expired leases
            }
        });

        // Main server loop
        self.serve().await?;

        Ok(())
    }

    async fn serve(&mut self) -> Result<()> {
        let socket = self.socket.as_ref().unwrap();
        let mut buf = [0u8; 1024];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    if let Ok(packet) = self.parse_dhcp_packet(&buf[..len]) {
                        self.handle_dhcp_packet(packet, src).await?;
                    }
                }
                Err(e) => {
                    warn!("DHCP server socket error: {}", e);
                }
            }
        }
    }

    async fn handle_dhcp_packet(&mut self, packet: DHCPPacket, _src: SocketAddr) -> Result<()> {
        let mac = &packet.chaddr[..6];
        let mac_array: [u8; 6] = mac.try_into().unwrap_or_default();

        // Find message type
        let message_type = packet.options.iter()
            .find_map(|opt| {
                if let DHCPOption::MessageType(msg_type) = opt {
                    Some(msg_type.clone())
                } else {
                    None
                }
            });

        match message_type {
            Some(DHCPMessageType::Discover) => {
                debug!("DHCP DISCOVER from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                       mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
                self.send_offer(packet, mac_array).await?;
            }
            Some(DHCPMessageType::Request) => {
                debug!("DHCP REQUEST from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                       mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
                self.send_ack(packet, mac_array).await?;
            }
            Some(DHCPMessageType::Release) => {
                debug!("DHCP RELEASE from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                       mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
                self.leases.remove(&mac_array);
            }
            _ => {
                debug!("Unhandled DHCP message type: {:?}", message_type);
            }
        }

        Ok(())
    }

    async fn send_offer(&mut self, request: DHCPPacket, mac: [u8; 6]) -> Result<()> {
        let offered_ip = self.allocate_ip(mac)?;

        let mut response = DHCPPacket {
            op: 2, // Reply
            htype: request.htype,
            hlen: request.hlen,
            hops: 0,
            xid: request.xid,
            secs: 0,
            flags: request.flags,
            ciaddr: Ipv4Addr::new(0, 0, 0, 0),
            yiaddr: offered_ip,
            siaddr: self.gateway,
            giaddr: request.giaddr,
            chaddr: request.chaddr,
            sname: [0; 64],
            file: [0; 128],
            options: vec![
                DHCPOption::MessageType(DHCPMessageType::Offer),
                DHCPOption::ServerIdentifier(self.gateway),
                DHCPOption::LeaseTime(self.lease_time),
                DHCPOption::SubnetMask("255.255.255.0".parse().unwrap()),
                DHCPOption::Router(vec![self.gateway]),
                DHCPOption::DomainNameServer(self.dns_servers.clone()),
                DHCPOption::End,
            ],
        };

        self.send_dhcp_packet(response).await?;
        info!("DHCP OFFER sent: {} to MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
              offered_ip, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);

        Ok(())
    }

    async fn send_ack(&mut self, request: DHCPPacket, mac: [u8; 6]) -> Result<()> {
        let ip = if let Some(lease) = self.leases.get(&mac) {
            lease.ip
        } else {
            self.allocate_ip(mac)?
        };

        // Create lease
        let lease = DHCPLease {
            ip,
            mac,
            hostname: None,
            expires: std::time::Instant::now() + Duration::from_secs(self.lease_time as u64),
        };
        self.leases.insert(mac, lease);

        let response = DHCPPacket {
            op: 2, // Reply
            htype: request.htype,
            hlen: request.hlen,
            hops: 0,
            xid: request.xid,
            secs: 0,
            flags: request.flags,
            ciaddr: request.ciaddr,
            yiaddr: ip,
            siaddr: self.gateway,
            giaddr: request.giaddr,
            chaddr: request.chaddr,
            sname: [0; 64],
            file: [0; 128],
            options: vec![
                DHCPOption::MessageType(DHCPMessageType::Ack),
                DHCPOption::ServerIdentifier(self.gateway),
                DHCPOption::LeaseTime(self.lease_time),
                DHCPOption::SubnetMask("255.255.255.0".parse().unwrap()),
                DHCPOption::Router(vec![self.gateway]),
                DHCPOption::DomainNameServer(self.dns_servers.clone()),
                DHCPOption::End,
            ],
        };

        self.send_dhcp_packet(response).await?;
        info!("DHCP ACK sent: {} to MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
              ip, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);

        Ok(())
    }

    async fn send_dhcp_packet(&self, packet: DHCPPacket) -> Result<()> {
        let data = self.serialize_dhcp_packet(packet)?;
        let broadcast_addr = SocketAddr::from(([255, 255, 255, 255], DHCP_CLIENT_PORT));

        if let Some(socket) = &self.socket {
            socket.send_to(&data, broadcast_addr)?;
        }

        Ok(())
    }

    fn allocate_ip(&mut self, mac: [u8; 6]) -> Result<Ipv4Addr> {
        // Check if we already have a lease
        if let Some(lease) = self.leases.get(&mac) {
            return Ok(lease.ip);
        }

        // Find available IP in pool
        let start_u32 = u32::from(self.pool_start);
        let end_u32 = u32::from(self.pool_end);

        for ip_u32 in start_u32..=end_u32 {
            let ip = Ipv4Addr::from(ip_u32);
            let in_use = self.leases.values().any(|lease| lease.ip == ip);

            if !in_use {
                return Ok(ip);
            }
        }

        Err(anyhow::anyhow!("No available IPs in DHCP pool"))
    }

    fn parse_dhcp_packet(&self, data: &[u8]) -> Result<DHCPPacket> {
        if data.len() < 240 {
            return Err(anyhow::anyhow!("DHCP packet too small"));
        }

        let mut chaddr = [0u8; 16];
        chaddr.copy_from_slice(&data[28..44]);

        let mut sname = [0u8; 64];
        sname.copy_from_slice(&data[44..108]);

        let mut file = [0u8; 128];
        file.copy_from_slice(&data[108..236]);

        // Parse options (simplified)
        let options = vec![DHCPOption::End];

        Ok(DHCPPacket {
            op: data[0],
            htype: data[1],
            hlen: data[2],
            hops: data[3],
            xid: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            secs: u16::from_be_bytes([data[8], data[9]]),
            flags: u16::from_be_bytes([data[10], data[11]]),
            ciaddr: Ipv4Addr::new(data[12], data[13], data[14], data[15]),
            yiaddr: Ipv4Addr::new(data[16], data[17], data[18], data[19]),
            siaddr: Ipv4Addr::new(data[20], data[21], data[22], data[23]),
            giaddr: Ipv4Addr::new(data[24], data[25], data[26], data[27]),
            chaddr,
            sname,
            file,
            options,
        })
    }

    fn serialize_dhcp_packet(&self, packet: DHCPPacket) -> Result<Vec<u8>> {
        let mut data = vec![0u8; 240];

        data[0] = packet.op;
        data[1] = packet.htype;
        data[2] = packet.hlen;
        data[3] = packet.hops;

        // XID
        let xid_bytes = packet.xid.to_be_bytes();
        data[4..8].copy_from_slice(&xid_bytes);

        // Secs
        let secs_bytes = packet.secs.to_be_bytes();
        data[8..10].copy_from_slice(&secs_bytes);

        // Flags
        let flags_bytes = packet.flags.to_be_bytes();
        data[10..12].copy_from_slice(&flags_bytes);

        // IP addresses
        data[12..16].copy_from_slice(&packet.ciaddr.octets());
        data[16..20].copy_from_slice(&packet.yiaddr.octets());
        data[20..24].copy_from_slice(&packet.siaddr.octets());
        data[24..28].copy_from_slice(&packet.giaddr.octets());

        // Hardware address
        data[28..44].copy_from_slice(&packet.chaddr);

        // Server name and file name
        data[44..108].copy_from_slice(&packet.sname);
        data[108..236].copy_from_slice(&packet.file);

        // Magic cookie for DHCP
        data[236] = 99;
        data[237] = 130;
        data[238] = 83;
        data[239] = 99;

        // Add options
        for option in packet.options {
            match option {
                DHCPOption::MessageType(msg_type) => {
                    data.extend_from_slice(&[53, 1, msg_type as u8]);
                }
                DHCPOption::ServerIdentifier(ip) => {
                    data.extend_from_slice(&[54, 4]);
                    data.extend_from_slice(&ip.octets());
                }
                DHCPOption::LeaseTime(time) => {
                    data.extend_from_slice(&[51, 4]);
                    data.extend_from_slice(&time.to_be_bytes());
                }
                DHCPOption::SubnetMask(mask) => {
                    data.extend_from_slice(&[1, 4]);
                    data.extend_from_slice(&mask.octets());
                }
                DHCPOption::Router(routers) => {
                    data.extend_from_slice(&[3, (routers.len() * 4) as u8]);
                    for router in routers {
                        data.extend_from_slice(&router.octets());
                    }
                }
                DHCPOption::DomainNameServer(servers) => {
                    data.extend_from_slice(&[6, (servers.len() * 4) as u8]);
                    for server in servers {
                        data.extend_from_slice(&server.octets());
                    }
                }
                DHCPOption::End => {
                    data.push(255);
                }
                _ => {}
            }
        }

        Ok(data)
    }
}