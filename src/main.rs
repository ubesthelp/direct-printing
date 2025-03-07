use api::Api;
use clap::Parser;
use log::info;
use poem::{listener::TcpListener, Route, Server};
use poem_openapi::OpenApiService;

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
}

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
  let args = Args::parse();

  pretty_env_logger::init();

  let addr = format!("{}:{}", args.host, args.port);
  let server = format!("http://{}/api", addr);

  let api_service = OpenApiService::new(Api, "Direct Printing", "0.1")
    .summary("直接打印 API")
    .description("可从 web 直接调用的打印 API。")
    .server(&server);

  #[cfg(feature = "with-ui")]
  let ui = api_service.swagger_ui();
  #[cfg(feature = "with-ui")]
  let spec = api_service.spec_endpoint_yaml();

  let app = Route::new().nest("/api", api_service);

  #[cfg(feature = "with-ui")]
  let app = app.nest("/", ui).nest("/spec", spec);

  info!("The API is served on {}", server);
  #[cfg(feature = "with-ui")]
  {
    info!("The documentation is served on http://{}/", addr);
    info!("The specification is served on http://{}/spec", addr);
  }

  Server::new(TcpListener::bind(addr)).run(app).await
}
