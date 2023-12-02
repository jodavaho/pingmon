
extern crate socket2;
extern crate libc;
use std::collections::HashMap;
use std::env;
use std::mem::MaybeUninit;
use std::net::SocketAddrV4;
use std::time::Duration;
use std::time::Instant;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use socket2::{Domain, Protocol, Socket, Type};

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
    let min_ttl = env::args().nth(2).unwrap_or("1".to_string()).parse::<u32>().unwrap();

    let max_hops = 30;

    //let socket = UdpSocket::bind("0.0.0.0:0").expect("Could not bind socket");
        //socket2::Socket::new (
        //socket2::Domain::IPV4,
        //socket2::Type::RAW,
        //Some(socket2::Protocol::ICMPV4)).unwrap();
    let socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)).unwrap();

    let timeout = Duration::from_secs(1);

    socket.set_read_timeout(Some(timeout)).expect("set_read_timeout call failed");
    socket.set_write_timeout(Some(timeout)).expect("set_write_timeout call failed");
    //socket.connect(&destination.into()).expect("connect call failed");
    let local_addr:SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0).into();
    socket.bind(&local_addr.into()).expect("bind call failed");
    let target_ip_s = target_ip.parse::<Ipv4Addr>().unwrap();
    let destination= SocketAddrV4::new(target_ip_s, 0);
    //socket.connect(&destination.into()).expect("connect call failed");

    let mut results: HashMap<u32, String> = HashMap::new();

    'ttl_loop: for ttl in min_ttl..=max_hops {
        socket.set_ttl(ttl).expect("set_ttl call failed");
        // Construct an ICMP echo request packet
        let mut packet = [0u8; 8];
        packet[0] = 8; // ICMP type: Echo Request
        packet[1] = 0; // ICMP code: 0
        packet[2..4].copy_from_slice(&[0, 0]); // ICMP checksum (set to 0 for now)
        packet[4..8].copy_from_slice(&[0, 0, 0, 0]); // Identifier and Sequence Number
        socket.send_to(&packet, &destination.into()).expect("send_to call failed");

        // Calculate ICMP checksum
        let checksum = calculate_checksum(&packet);
        packet[2..4].copy_from_slice(&checksum.to_be_bytes());
        // Send the ICMP echo request packet
        let send_time = Instant::now();
        println!("Hop {}: Request Sent", ttl);
        // Receive ICMP echo response packets
        let mut response_packet = [MaybeUninit::<u8>::uninit(); 64];
        match socket.recv(&mut response_packet) {
            Ok(_) => {
                let recv_time = Instant::now();
                let r = recv_time.duration_since(send_time);
                let r_ms = r.as_secs() * 1000 + r.subsec_millis() as u64;
                let response_packet = unsafe {
                    std::mem::transmute::<[MaybeUninit<u8>; 64], [u8; 64]>(response_packet)
                };
                let response_packet = &response_packet[..];
                //print in hex:
                for i in 0..response_packet.len() {
                    print!("{:02X} ", response_packet[i]);
                    if i % 16 == 15 {
                        println!();
                    }
                }
                //get sender ip
                let sender_ip = Ipv4Addr::new(response_packet[12], response_packet[13], response_packet[14], response_packet[15]);
                println!("Hop {}: Response Received in {} ms from {} (target: {})", ttl, r_ms, sender_ip, target_ip);
                if sender_ip.to_string() == target_ip {
                    println!("Target Reached");
                    break 'ttl_loop;
                }
            }
            Err(x) => {
                println!("Hop {}: Request Timed Out", ttl);
            }
        }

    }

}
