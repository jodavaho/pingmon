
use std::net::Ipv4Addr;
use std::net::IpAddr;
use std::thread;
use tracert::trace::Tracer;
use serde::{Serialize, Deserialize};
use serde_json::json;

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


    let dest_str = std::env::args().nth(1).expect("No destination specified");
    let destination = IpAddr::V4(dest_str.parse::<Ipv4Addr>().unwrap());
    let tracer:Tracer = Tracer::new(destination).expect("Error creating tracer");

    let handle = thread::spawn(move || tracer.trace());


    let hop_list :Vec<SHop> = match handle.join(){
        Ok(result) => {
            result.unwrap()
                .nodes.iter()
                .map(|n| 
                     SHop{ 
                         rtt:n.rtt.as_micros(), 
                         seq:n.seq.into(), 
                         host:n.host_name.to_owned(), 
                         ip:n.ip_addr.to_string() 
                     }).collect()
        }
        Err(e) => {
            eprintln!("Error: {:?}", e); 
            Vec::new()
        }
    };

    println!("{}", json!(&hop_list));


}
