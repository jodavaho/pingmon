use log;
use log::{error, info, warn,debug};
use std::net::IpAddr;
use std::thread;
use tracert::trace::Tracer;
use serde::{Serialize, Deserialize};
use serde_json::json;
use argp::FromArgs;
use std::path::PathBuf;
use atty::Stream;
use env_logger::Env;

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
struct SHop
{
    rtt:u128,
    seq:i32,
    host:String,
    ip:String
}

#[derive(Debug,FromArgs)]
/// Traceroute utility - monitors connectivity and latency to a destination
struct CliArgs{
    #[argp(option, short='c', long="config")]
    /// Path to config file
    config_file:Option<PathBuf>,

    #[argp(positional)]
    /// list of hosts to trace
    hosts:Vec<String>,

    #[argp(switch, short='v', long="verbose")]
    /// Verbose output, specify multiple times for more verbosity. Also honors PINGMON_LOG environment variable set to a log level
    verbose:i32,


}

fn get_default_host_list_path()->PathBuf{
    directories::ProjectDirs::from("io", "jodavaho", "pingmon")
        .unwrap()
        .config_dir()
        .join("hosts.txt")
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
struct CliConfig{
    #[serde(default="get_default_host_list_path")]
    host_list_file:PathBuf,

    hosts:Vec<String>

}

impl Default for CliConfig{
    fn default()->Self{
        CliConfig{
            host_list_file:get_default_host_list_path(),
            hosts:Vec::new()
        }
    }
}
impl CliConfig{
    fn write_to_file(&self, path:&PathBuf)->Result<(),std::io::Error>{
        let toml = toml::to_string(self).expect("Error serializing config");
        std::fs::write(path, toml)
    }
}

fn main() {

    let args= argp::parse_args_or_exit::<CliArgs>(argp::DEFAULT);
    if atty::is(Stream::Stdout){
        println!("\x1b[2J\x1b[1;1H");
        let default_level = match args.verbose{
            0 => "error",
            1 => "warn",
            2 => "info",
            3 => "debug",
            _ => "trace"
        };
        env_logger::Builder::from_env(Env::default().filter_or("PINGMON_LOG", default_level)).init();
        //env_logger::Builder::from_env(Env::default().default_filter_or(default_level)).init();
    } else {
        log::set_max_level(log::LevelFilter::Off);
    }
    eprintln!("Pingmon - a traceroute utility. https://pingmon.jodavaho.io/");



    info!("Starting up");
    let project_dir = directories::ProjectDirs::from("io", "jodavaho", "pingmon").unwrap();
    debug!("Project dir: {:?}", project_dir);
    let config_dir = project_dir.config_dir();

    // The hard-coded defaults to start with
    let mut cfg = CliConfig::default();

    debug!("Config dir: {}", config_dir.to_str().unwrap_or("Unknown"));

    // Check for, and create, the default config file 
    let default_config_file = config_dir.join("config.toml");
    if !default_config_file.exists(){
        debug!("Creating default config file: {:?}", default_config_file);
        std::fs::create_dir_all(config_dir).unwrap_or_else(|e| {
            warn!("Error creating config directory: {:?} - proceeding", e);
        });
        cfg.write_to_file(&default_config_file)
            .unwrap_or_else(|e| {
                warn!("Error writing default config file: {:?} - proceeding", e);
            });
    } else {
        debug!("Found default config file: {:?}", default_config_file);
    }

    // Load the default config file, or use the defaults we created if you can't
    cfg = toml::from_str(
        &std::fs::read_to_string(&default_config_file)
        .or_else(|e| {
            error!("Error reading default config file: {:?}", e);
            toml::to_string(&cfg)
        }).unwrap()).unwrap();


    // See if they passed in a config file, and if not, use the default
    if args.config_file.is_some(){
        let config_file = args.config_file.as_ref().unwrap();
        debug!("Found user-specified config file: {:?}", &config_file);
        cfg = toml::from_str(
            &std::fs::read_to_string(&config_file)
            .or_else(|e| {
                error!("Error reading specified config file: {}, defaulting", e);
                toml::to_string(&cfg)
            }).unwrap()).unwrap();
    } 

    let mut host_list = cfg.hosts;
    if args.hosts.len() > 0{
        host_list.extend(args.hosts);
    }

    debug!("Host list: {:?}", host_list);

    for destination in host_list{

        let destination = match destination.parse::<IpAddr>()
        {
            Ok(ip) => ip,
            Err(e) => {
                error!("Error parsing destination: {}, {}", destination, e);
                continue
            }
        };

        let tracer:Tracer = match Tracer::new(destination)
        {
            Ok(t) => t,
            Err(e) => {
                error!("Error creating tracer: {}", e);
                continue
            }
        };

        let handle = thread::spawn(move || tracer.trace());

        let hop_list :Vec<SHop> = match handle.join(){
            Ok(result) => {
                result.unwrap()
                    .nodes.iter()
                    .map(|n| 
                         {
                             debug!("{:?}", n);
                             SHop{ 
                                 rtt:n.rtt.as_micros(), 
                                 seq:n.seq.into(), 
                                 host:n.host_name.to_owned(), 
                                 ip:n.ip_addr.to_string() 
                             }
                         }).collect()

            }
            Err(e) => {
                error!("Error in thread: {}", 
                       e.downcast_ref::<String>()
                       .unwrap_or(&"Unknown error".to_string())
                      );
                Vec::new()
            }
        };

        println!("{}", json!(&hop_list));
    }
    eprintln!("Done");
}


