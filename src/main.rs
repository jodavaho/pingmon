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

#[derive(Debug,Serialize, Deserialize, Clone)]
struct SHop
{
    rtt:u64,
    seq:u64,
    host:String,
    ip:String,
    timeout:bool,
    final_dest:String,
    node_type:String,
    time:DateTime<Utc>
}

impl SHop{
    fn to_line_protocol_v2(&self, measurement:&str)->String{
        format!("{} seq={},host=\"{}\",ip=\"{}\",timeout={},final_dest=\"{}\",node_type=\"{}\" {} {}",
                measurement,
                self.seq,
                self.host,
                self.ip,
                self.timeout,
                self.final_dest,
                self.node_type,
                self.rtt,
                self.time.timestamp_nanos_opt().unwrap_or(
                    self.time.timestamp_micros() as i64 * 1_000
                ),
                )
    }

}

fn batch_to_line(batch:Vec<SHop>, measurement:&str)->String{
    batch.iter()
        .map(|h| h.to_line_protocol_v2(measurement))
        .collect::<Vec<String>>()
        .join("\n")
}

async fn post_to_influxdb2(api_key:&str, host:&str, org:&str, bucket:&str, batch:Vec<SHop>)->Result<(),reqwest::Error>{
    let client = reqwest::Client::new();
    let url = format!("{}/api/v2/write?org={}&bucket={}&precision=ns", host, org, bucket);
    let data = batch_to_line(batch, "pingmon");
    let res = client.post(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "text/plain")
        .body(data)
        .send().await?;
    info!("InfluxDB response: {:?}", res);
    Ok(())
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
    influxdb_api_key:Option<String>,

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

    influxdb_api_key:Option<String>,

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
            influxdb_api_key:None,
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

    let influxdb_api_key:String = match args.influxdb_api_key{
        Some(k) => k,
        None => cfg.influxdb_api_key.unwrap_or("".to_string())
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
    debug!("Influx token: {}", influxdb_api_key);


    //let full_host = format!("{}:{}", influxdb_host, influxdb_port);
    let full_host = format!("{}", influxdb_host);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async{
        debug!("Writing to influx");
        for hops in results{
            debug!("Writing hops to dest {}", hops[0].final_dest);
            let client = influxdb::Client::new(&full_host, &influxdb_bucket).with_token(&influxdb_api_key);
            debug!("{:?}", client);
            //debug!("{:?}", client.ping().await);
            tokio::spawn(async move {
                debug!("async writing to influx");
                let query:Vec<influxdb::WriteQuery> = hops.iter().map(|h| h.clone().into_query("ping")).collect();
                debug!("{:?}", query);
                match client.query(&query).await{
                    Ok(_) => info!("Wrote to influx"),
                    Err(e) => error!("Error writing to influx: {}", e)
                }
            }).await.unwrap();
        }
    });

}
