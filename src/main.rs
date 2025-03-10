use std::fs::write;

use api::Api;
use clap::Parser;
use log::info;
use poem::{listener::TcpListener, EndpointExt, Route, Server};
use poem_openapi::OpenApiService;

#[cfg(feature = "with-ui")]
use poem::middleware::{RequestId, ReuseId, Tracing};

mod api;

/// Direct Printing
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  /// The host to listen
  #[arg(long, default_value = "127.0.0.1")]
  host: String,

  /// The port to listen
  #[arg(short, long, default_value_t = 63856)]
  port: u16,

  /// Save the OpenAPI specification into a JSON file
  #[arg(long, value_name = "FILE")]
  json: Option<String>,

  /// Save the OpenAPI specification into a YAML file
  #[arg(long, value_name = "FILE")]
  yaml: Option<String>,
}

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
  let args = Args::parse();

  #[cfg(feature = "with-ui")]
  tracing_subscriber::fmt::init();

  let addr = format!("{}:{}", args.host, args.port);
  let server = format!("http://{}/api", addr);

  let api_service = OpenApiService::new(Api, "Direct Printing", "0.1")
    .description("可从 web 直接调用的打印 API。")
    .server(&server);

  if let Some(json) = args.json {
    write(json, api_service.spec())
  } else if let Some(yaml) = args.yaml {
    write(yaml, api_service.spec_yaml())
  } else {
    #[cfg(feature = "with-ui")]
    let ui = api_service.swagger_ui();
    #[cfg(feature = "with-ui")]
    let spec = api_service.spec_endpoint_yaml();

    let app = Route::new().nest("/api", api_service);

    #[cfg(feature = "with-ui")]
    let app = app
      .nest("/", ui)
      .nest("/spec", spec)
      .with(Tracing)
      .with(RequestId::default().reuse_id(ReuseId::Use));

    info!("The API is served on {}", server);
    #[cfg(feature = "with-ui")]
    {
      info!("The documentation is served on http://{}/", addr);
      info!("The specification is served on http://{}/spec", addr);
    }

    Server::new(TcpListener::bind(addr)).run(app).await
  }
}
