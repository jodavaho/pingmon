
extern crate socket2;
extern crate libc;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddrV4;
use std::time::Duration;
use std::time::Instant;
use std::net::UdpSocket;
use std::net::Ipv4Addr;

fn calculate_checksum(packet: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0;
    while i < packet.len() {
        let word = (packet[i] as u32) << 8 | (packet[i + 1] as u32);
        sum += word;
        i += 2;
    }
    while sum >> 16 != 0 {
        sum = (sum >> 16) + (sum & 0xffff);
    }
    !(sum as u16)
}

fn main() {
    let target_ip = env::args().nth(1).unwrap_or("192.168.1.1".to_string());

    let max_hops = 30;

    let socket = UdpSocket::bind("0.0.0.0:0").expect("Could not bind socket");
        //socket2::Socket::new (
        //socket2::Domain::IPV4,
        //socket2::Type::RAW,
        //Some(socket2::Protocol::ICMPV4)).unwrap();

    let timeout = Duration::from_secs(5);

    socket.set_read_timeout(Some(timeout)).expect("set_read_timeout call failed");
    socket.set_write_timeout(Some(timeout)).expect("set_write_timeout call failed");

    let mut results: HashMap<u32, String> = HashMap::new();

    for ttl in 1..=max_hops {
        socket.set_ttl(ttl).expect("set_ttl call failed");
        // Construct an ICMP echo request packet
        let mut packet = [0u8; 8];
        packet[0] = 8; // ICMP type: Echo Request
        packet[1] = 0; // ICMP code: 0
        packet[2..4].copy_from_slice(&[0, 0]); // ICMP checksum (set to 0 for now)
        packet[4..8].copy_from_slice(&[0, 0, 0, 0]); // Identifier and Sequence Number

        // Calculate ICMP checksum
        let checksum = calculate_checksum(&packet);
        packet[2..4].copy_from_slice(&checksum.to_be_bytes());

        //convert string to ip address
        let target_ip_s = target_ip.parse::<Ipv4Addr>().unwrap();

        // Send the ICMP echo request packet
        let destination= SocketAddrV4::new(target_ip_s, 7);
        socket.send_to(&packet, destination).expect("send_to call failed");

        println!("Hop {}: Request Sent", ttl);

        // Receive ICMP echo response packets
        let mut response_packet = [0u8; 28];
        match socket.recv_from(response_packet.as_mut_slice()) {
            Ok((_, source_addr)) => {
                // Handle the response packet (e.g., extract IP address, measure latency)
                let now = Instant::now();
                let latency = now.elapsed().as_millis();
                let hop_info = format!("Hop {}: {} ({}ms)", ttl, source_addr.ip(), latency);
                println!("{}", hop_info);
                results.insert(ttl, hop_info);

                // Check if the response came from the target
                if target_ip == source_addr.ip().to_string() {
                    break; // Target reached
                }
            }
            Err(_) => {
                println!("Hop {}: Request Timed Out", ttl);
            }
        }

    }

}
