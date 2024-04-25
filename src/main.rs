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
use chrono::{DateTime,Utc};
use influxdb::InfluxDbWriteable;

#[derive(Debug,Serialize, Deserialize, Clone, InfluxDbWriteable)]
struct SHop
{
    rtt:u64,
    #[influxdb(tag)]
    seq:u64,
    #[influxdb(tag)]
    host:String,
    #[influxdb(tag)]
    ip:String,
    timeout:bool,
    #[influxdb(tag)]
    final_dest:String,
    #[influxdb(tag)]
    node_type:String,
    time:DateTime<Utc>
}

impl SHop{
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

    #[argp(option, short='k', long="api-key")]
    /// InfluxDB API key
    api_key:Option<String>,

    #[argp(option, long="influx-host")]
    /// InfluxDB host
    influxdb_host:Option<String>,

    #[argp(option, long="influx-org")]
    /// InfluxDB organization
    influxdb_org:Option<String>,

    #[argp(option, long="influx-port")]
    /// InfluxDB port
    influxdb_port:Option<u16>,

    #[argp(option, long="influx-bucket")]
    /// InfluxDB bucket
    influxdb_bucket:Option<String>,
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

    hosts:Vec<String>,

    api_key:Option<String>,

    influxdb_host:Option<String>,

    influxdb_org:Option<String>,

    influxdb_port:Option<u16>,

    influxdb_bucket:Option<String>,

}

impl Default for CliConfig{
    fn default()->Self{
        CliConfig{
            host_list_file:get_default_host_list_path(),
            hosts:Vec::new(),
            api_key:None,
            influxdb_host:None,
            influxdb_org:None,
            influxdb_port:None,
            influxdb_bucket:None
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
    let mut results = Vec::<Vec::<SHop>>::new();

    //todo make this a thread pool or something
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

        let start_time = Utc::now();
        let handle = thread::spawn(move || tracer.trace());

        let hop_list :Vec<SHop> = match handle.join(){
            Ok(result) => {
                result.unwrap()
                    .nodes.iter()
                    .map(|n|
                         {
                             debug!("{:?}", n);
                             SHop{
                                 rtt:n.rtt.as_micros() as u64 * 1000,
                                 seq:n.seq.into(),
                                 host:n.host_name.to_owned(),
                                 ip:n.ip_addr.to_string(),
                                 timeout:n.rtt.as_micros() == 0,
                                 final_dest:destination.to_string(),
                                 node_type:
                                     match n.node_type{
                                         tracert::node::NodeType::DefaultGateway => "DefaultGateway".to_string(),
                                         tracert::node::NodeType::Relay => "Relay".to_string(),
                                         tracert::node::NodeType::Destination => "Destination".to_string(),
                                     },
                                 time:start_time,

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
        results.push(hop_list.clone());
        println!("{}", json!(&hop_list));
    }

    let api_key:String = match args.api_key{
        Some(k) => k,
        None => cfg.api_key.unwrap_or("".to_string())
    };

    let influxdb_host = args.influxdb_host
        .unwrap_or(
            cfg.influxdb_host
            .unwrap_or("http://localhost:8086".to_string())
            );

    let influxdb_org = args.influxdb_org
        .unwrap_or(
            cfg.influxdb_org
            .unwrap_or("none".to_string())
            );

    let influxdb_port = args.influxdb_port
        .unwrap_or(
            cfg.influxdb_port
            .unwrap_or(8086)
            );

    let influxdb_bucket = args.influxdb_bucket
        .unwrap_or(
            cfg.influxdb_bucket
            .unwrap_or("pingmon".to_string())
            );

    debug!("Influx host: {}", influxdb_host);
    debug!("Influx org: {}", influxdb_org);
    debug!("Influx port: {}", influxdb_port);
    debug!("Influx bucket: {}", influxdb_bucket);

    //let full_host = format!("{}:{}", influxdb_host, influxdb_port);
    let full_host = format!("{}", influxdb_host);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async{
        debug!("Writing to influx");
        for hops in results{
            debug!("Writing hops to dest {}", hops[0].final_dest);
            let client = influxdb::Client::new(&full_host, &influxdb_bucket).with_token(&api_key);
            debug!("{:?}", client);
            //debug!("{:?}", client.ping().await);
            tokio::spawn(async move {
                debug!("async writing to influx");
                let query:Vec<influxdb::WriteQuery> = hops.iter().map(|h| h.clone().into_query("trace")).collect();
                debug!("{:?}", query);
                match client.query(&query).await{
                    Ok(_) => info!("Wrote to influx"),
                    Err(e) => error!("Error writing to influx: {}", e)
                }
            }).await.unwrap();
        }
    });

}
