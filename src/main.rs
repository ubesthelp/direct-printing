use api::Api;
use poem::{listener::TcpListener, Route, Server};
use poem_openapi::OpenApiService;

mod api;

#[tokio::main]
async fn main() {
  pretty_env_logger::init();

  let api_service = OpenApiService::new(Api, "Direct Printing", "0.1")
    .summary("直接打印 API")
    .description("可从 web 直接调用的打印 API。")
    .server("http://127.0.0.1:63856");

  #[cfg(feature = "with-ui")]
  let ui = api_service.swagger_ui();
  #[cfg(feature = "with-ui")]
  let spec = api_service.spec_endpoint_yaml();

  let app = Route::new().nest("/api", api_service);

  #[cfg(feature = "with-ui")]
  let app = app.nest("/", ui).nest("/spec", spec);

  let _ = Server::new(TcpListener::bind("127.0.0.1:63856"))
    .run(app)
    .await;
}
