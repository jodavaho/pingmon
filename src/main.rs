
use std::net::Ipv4Addr;
use std::net::IpAddr;
use std::thread;
use tracert::trace::Tracer;
use serde::{Serialize, Deserialize};

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
struct SHop
{
    rtt:u128,
    seq:i32,
    host:String,
    ip:String
}

fn main() {
    let dest_str = "34.174.155.119";
    let destination = IpAddr::V4(dest_str.parse::<Ipv4Addr>().unwrap());
    let tracer:Tracer = Tracer::new(destination).expect("Error creating tracer");

    let handle = thread::spawn(move || tracer.trace());

    match handle.join(){
        Ok(result) => {
            let nodes = result.unwrap().nodes;
            for n in nodes
            {
                let s = SHop{ rtt:n.rtt.as_micros(), seq:n.seq.into(), host:n.host_name, ip:n.ip_addr.to_string() };
                println!("{}", serde_json::to_string(&s).unwrap());
            }
        }
        Err(e) => eprintln!("Error: {:?}", e),
    }
}
