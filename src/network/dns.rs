use anyhow::Result;
use std::net::{UdpSocket, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{debug, info};
use crate::database::Database;

const DNS_PORT: u16 = 53;

#[derive(Debug, Clone)]
pub struct DNSServer {
    database: Arc<Database>,
    socket: Option<UdpSocket>,
    records: HashMap<String, Vec<DNSRecord>>,
    forwarders: Vec<Ipv4Addr>,
}

#[derive(Debug, Clone)]
struct DNSRecord {
    name: String,
    record_type: DNSRecordType,
    ttl: u32,
    data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
enum DNSRecordType {
    A = 1,
    NS = 2,
    CNAME = 5,
    PTR = 12,
    MX = 15,
    TXT = 16,
    AAAA = 28,
}

#[derive(Debug)]
struct DNSPacket {
    header: DNSHeader,
    questions: Vec<DNSQuestion>,
    answers: Vec<DNSAnswer>,
    authorities: Vec<DNSAnswer>,
    additionals: Vec<DNSAnswer>,
}

#[derive(Debug)]
struct DNSHeader {
    id: u16,
    flags: u16,
    questions: u16,
    answers: u16,
    authorities: u16,
    additionals: u16,
}

#[derive(Debug)]
struct DNSQuestion {
    name: String,
    qtype: u16,
    qclass: u16,
}

#[derive(Debug)]
struct DNSAnswer {
    name: String,
    answer_type: u16,
    class: u16,
    ttl: u32,
    length: u16,
    data: Vec<u8>,
}

impl DNSServer {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            socket: None,
            records: HashMap::new(),
            forwarders: vec![
                "8.8.8.8".parse().unwrap(),
                "1.1.1.1".parse().unwrap(),
            ],
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting DNS server on port {}", DNS_PORT);

        // Bind to DNS port
        let addr = SocketAddr::from(([0, 0, 0, 0], DNS_PORT));
        let socket = UdpSocket::bind(addr)?;
        self.socket = Some(socket);

        // Load DNS records from database
        self.load_records().await?;

        // Add default records
        self.add_default_records().await?;

        // Start server loop
        self.serve().await?;

        Ok(())
    }

    async fn serve(&mut self) -> Result<()> {
        let socket = self.socket.as_ref().unwrap();
        let mut buf = [0u8; 512];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    if let Ok(query) = self.parse_dns_packet(&buf[..len]) {
                        let response = self.handle_dns_query(query).await?;
                        let response_data = self.serialize_dns_packet(response)?;
                        socket.send_to(&response_data, src)?;
                    }
                }
                Err(e) => {
                    debug!("DNS server socket error: {}", e);
                }
            }
        }
    }

    async fn handle_dns_query(&self, query: DNSPacket) -> Result<DNSPacket> {
        let mut response = DNSPacket {
            header: DNSHeader {
                id: query.header.id,
                flags: 0x8000, // Response flag
                questions: query.header.questions,
                answers: 0,
                authorities: 0,
                additionals: 0,
            },
            questions: query.questions.clone(),
            answers: Vec::new(),
            authorities: Vec::new(),
            additionals: Vec::new(),
        };

        for question in &query.questions {
            if let Some(records) = self.records.get(&question.name.to_lowercase()) {
                for record in records {
                    if record.record_type as u16 == question.qtype {
                        response.answers.push(DNSAnswer {
                            name: record.name.clone(),
                            answer_type: record.record_type as u16,
                            class: 1, // IN
                            ttl: record.ttl,
                            length: record.data.len() as u16,
                            data: record.data.clone(),
                        });
                    }
                }
            } else {
                // Forward to upstream DNS if not found locally
                if let Ok(forwarded_response) = self.forward_query(&query).await {
                    return Ok(forwarded_response);
                }
            }
        }

        response.header.answers = response.answers.len() as u16;

        // Set response code
        if response.answers.is_empty() {
            response.header.flags |= 3; // NXDOMAIN
        }

        Ok(response)
    }

    async fn forward_query(&self, query: &DNSPacket) -> Result<DNSPacket> {
        // Simple forwarding to upstream DNS
        let query_data = self.serialize_dns_packet(query.clone())?;

        for forwarder in &self.forwarders {
            let addr = SocketAddr::from((*forwarder, DNS_PORT));

            if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
                if socket.send_to(&query_data, addr).is_ok() {
                    let mut buf = [0u8; 512];
                    if let Ok((len, _)) = socket.recv_from(&mut buf) {
                        if let Ok(response) = self.parse_dns_packet(&buf[..len]) {
                            return Ok(response);
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Failed to forward DNS query"))
    }

    async fn load_records(&mut self) -> Result<()> {
        // TODO: Load DNS records from database
        Ok(())
    }

    async fn add_default_records(&mut self) -> Result<()> {
        // Add localhost records
        self.add_a_record("localhost", "127.0.0.1", 300)?;

        // Add reverse DNS for localhost
        self.add_ptr_record("1.0.0.127.in-addr.arpa", "localhost", 300)?;

        // Add CasVPS management interface
        let gateway = self.database.get_config("network.gateway").await?;
        if !gateway.is_empty() {
            self.add_a_record("casvps.local", &gateway, 300)?;
            self.add_a_record("admin.casvps.local", &gateway, 300)?;
        }

        Ok(())
    }

    fn add_a_record(&mut self, name: &str, ip: &str, ttl: u32) -> Result<()> {
        let ip_addr: Ipv4Addr = ip.parse()?;
        let record = DNSRecord {
            name: name.to_string(),
            record_type: DNSRecordType::A,
            ttl,
            data: ip_addr.octets().to_vec(),
        };

        self.records.entry(name.to_lowercase())
            .or_insert_with(Vec::new)
            .push(record);

        debug!("Added A record: {} -> {}", name, ip);
        Ok(())
    }

    fn add_ptr_record(&mut self, name: &str, hostname: &str, ttl: u32) -> Result<()> {
        let hostname_encoded = self.encode_domain_name(hostname);
        let record = DNSRecord {
            name: name.to_string(),
            record_type: DNSRecordType::PTR,
            ttl,
            data: hostname_encoded,
        };

        self.records.entry(name.to_lowercase())
            .or_insert_with(Vec::new)
            .push(record);

        debug!("Added PTR record: {} -> {}", name, hostname);
        Ok(())
    }

    fn encode_domain_name(&self, name: &str) -> Vec<u8> {
        let mut encoded = Vec::new();

        for part in name.split('.') {
            encoded.push(part.len() as u8);
            encoded.extend_from_slice(part.as_bytes());
        }

        encoded.push(0); // Null terminator
        encoded
    }

    fn parse_dns_packet(&self, data: &[u8]) -> Result<DNSPacket> {
        if data.len() < 12 {
            return Err(anyhow::anyhow!("DNS packet too small"));
        }

        let header = DNSHeader {
            id: u16::from_be_bytes([data[0], data[1]]),
            flags: u16::from_be_bytes([data[2], data[3]]),
            questions: u16::from_be_bytes([data[4], data[5]]),
            answers: u16::from_be_bytes([data[6], data[7]]),
            authorities: u16::from_be_bytes([data[8], data[9]]),
            additionals: u16::from_be_bytes([data[10], data[11]]),
        };

        let mut offset = 12;
        let mut questions = Vec::new();

        // Parse questions
        for _ in 0..header.questions {
            let (question, new_offset) = self.parse_question(data, offset)?;
            questions.push(question);
            offset = new_offset;
        }

        Ok(DNSPacket {
            header,
            questions,
            answers: Vec::new(),
            authorities: Vec::new(),
            additionals: Vec::new(),
        })
    }

    fn parse_question(&self, data: &[u8], mut offset: usize) -> Result<(DNSQuestion, usize)> {
        let (name, new_offset) = self.parse_domain_name(data, offset)?;
        offset = new_offset;

        if offset + 4 > data.len() {
            return Err(anyhow::anyhow!("Invalid question format"));
        }

        let qtype = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let qclass = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);

        Ok((DNSQuestion { name, qtype, qclass }, offset + 4))
    }

    fn parse_domain_name(&self, data: &[u8], mut offset: usize) -> Result<(String, usize)> {
        let mut name = String::new();
        let mut jumped = false;
        let mut jumps = 0;
        let original_offset = offset;

        loop {
            if offset >= data.len() {
                return Err(anyhow::anyhow!("Invalid domain name"));
            }

            let length = data[offset];

            if length == 0 {
                offset += 1;
                break;
            }

            // Check for compression (pointer)
            if (length & 0xc0) == 0xc0 {
                if !jumped {
                    offset += 2;
                }

                let pointer = u16::from_be_bytes([data[offset - 1] & 0x3f, data[offset]]);
                offset = pointer as usize;
                jumped = true;
                jumps += 1;

                if jumps > 10 {
                    return Err(anyhow::anyhow!("Too many DNS compression jumps"));
                }
            } else {
                offset += 1;

                if offset + length as usize > data.len() {
                    return Err(anyhow::anyhow!("Invalid domain name length"));
                }

                if !name.is_empty() {
                    name.push('.');
                }

                let part = std::str::from_utf8(&data[offset..offset + length as usize])?;
                name.push_str(part);
                offset += length as usize;
            }
        }

        if jumped {
            Ok((name, original_offset + 2))
        } else {
            Ok((name, offset))
        }
    }

    fn serialize_dns_packet(&self, packet: DNSPacket) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // Header
        data.extend_from_slice(&packet.header.id.to_be_bytes());
        data.extend_from_slice(&packet.header.flags.to_be_bytes());
        data.extend_from_slice(&packet.header.questions.to_be_bytes());
        data.extend_from_slice(&packet.header.answers.to_be_bytes());
        data.extend_from_slice(&packet.header.authorities.to_be_bytes());
        data.extend_from_slice(&packet.header.additionals.to_be_bytes());

        // Questions
        for question in &packet.questions {
            data.extend_from_slice(&self.encode_domain_name(&question.name));
            data.extend_from_slice(&question.qtype.to_be_bytes());
            data.extend_from_slice(&question.qclass.to_be_bytes());
        }

        // Answers
        for answer in &packet.answers {
            data.extend_from_slice(&self.encode_domain_name(&answer.name));
            data.extend_from_slice(&answer.answer_type.to_be_bytes());
            data.extend_from_slice(&answer.class.to_be_bytes());
            data.extend_from_slice(&answer.ttl.to_be_bytes());
            data.extend_from_slice(&answer.length.to_be_bytes());
            data.extend_from_slice(&answer.data);
        }

        Ok(data)
    }
}