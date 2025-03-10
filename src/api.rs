use std::{fs::read_to_string, io::Write, path::PathBuf};

use anyhow::bail;
use directories::ProjectDirs;
use log::{debug, error};
use poem::{error::InternalServerError, Result};
use poem_openapi::{
  param::Path,
  payload::Json,
  types::{Base64, ParseFromJSON, ToJSON},
  Enum, Object, OpenApi, Tags,
};
use tempfile::NamedTempFile;
use winprint::{
  printer::{FilePrinter, PdfiumPrinter, PrinterDevice},
  ticket::{
    Copies, FeatureOptionPack, FeatureOptionPackWithPredefined, PredefinedPageOrientation,
    PrintCapabilities, PrintTicketBuilder,
  },
};

/// 统一响应
#[derive(Object)]
#[oai(skip_serializing_if_is_none)]
struct Response<T>
where
  T: ParseFromJSON + ToJSON,
{
  /// 代码，0 表示成功，非 0 表示失败
  code: i32,
  /// 错误消息
  msg: Option<String>,
  /// 成功时的数据
  data: Option<T>,
}

impl<T> Response<T>
where
  T: ParseFromJSON + ToJSON,
{
  fn ok(data: T) -> Json<Self> {
    Json(Self {
      code: 0,
      msg: None,
      data: Some(data),
    })
  }

  fn err(msg: impl ToString) -> Json<Self> {
    Json(Self {
      code: 1,
      msg: Some(msg.to_string()),
      data: None,
    })
  }
}

/// 布局
#[derive(Debug, Enum)]
#[oai(rename_all = "snake_case")]
enum Orientation {
  /// 纵向
  Portrait,
  /// 横向
  Landscape,
  ReversePortrait,
  ReverseLandscape,
}

impl From<PredefinedPageOrientation> for Orientation {
  fn from(value: PredefinedPageOrientation) -> Self {
    match value {
      PredefinedPageOrientation::Portrait => Orientation::Portrait,
      PredefinedPageOrientation::Landscape => Orientation::Landscape,
      PredefinedPageOrientation::ReversePortrait => Orientation::ReversePortrait,
      PredefinedPageOrientation::ReverseLandscape => Orientation::ReverseLandscape,
    }
  }
}

impl From<&Orientation> for PredefinedPageOrientation {
  fn from(value: &Orientation) -> Self {
    match value {
      Orientation::Portrait => PredefinedPageOrientation::Portrait,
      Orientation::Landscape => PredefinedPageOrientation::Landscape,
      Orientation::ReversePortrait => PredefinedPageOrientation::ReversePortrait,
      Orientation::ReverseLandscape => PredefinedPageOrientation::ReverseLandscape,
    }
  }
}

/// 纸张大小
#[derive(Debug, Object)]
#[oai(skip_serializing_if_is_none)]
struct PageSize {
  /// 名称
  name: Option<String>,
  /// 宽度，微米
  width: u32,
  /// 高度，微米
  height: u32,
}

/// 打印机能力
#[derive(Object)]
#[oai(skip_serializing_if_is_none)]
struct PrinterCapability {
  /// 最大打印份数
  max_copies: Option<u16>,
  /// 布局
  orientations: Option<Vec<Orientation>>,
  /// 纸张大小
  page_sizes: Option<Vec<PageSize>>,
}

/// 打印设置
#[derive(Debug, Object)]
#[oai(skip_serializing_if_is_none)]
struct PrintSettings {
  /// 要使用的打印机名称
  printer: String,
  /// 打印份数
  copies: Option<u16>,
  /// 布局
  orientation: Option<Orientation>,
  /// 纸张大小
  page_size: Option<PageSize>,
}

/// 打印负载
#[derive(Object)]
#[oai(skip_serializing_if_is_none)]
struct PrintPayload {
  /// 要打印的 PDF 文件内容
  file: Base64<Vec<u8>>,
  /// 打印设置
  settings: PrintSettings,
}

#[derive(Tags)]
enum ApiTag {
  /// 打印 API
  Printing,
}

pub struct Api;

#[OpenApi(tag = "ApiTag::Printing")]
impl Api {
  /// 获取全部可用打印机名称列表。
  #[oai(path = "/printers", method = "get", operation_id = "getPrinters")]
  async fn get_printers(&self) -> Json<Response<Vec<String>>> {
    let printers = PrinterDevice::all().unwrap_or_default();
    Response::ok(printers.iter().map(|p| p.name().to_string()).collect())
  }

  /// 获取指定打印机能力。
  #[oai(path = "/printers/:name", method = "get", operation_id = "getPrinter")]
  async fn get_printer(&self, name: Path<String>) -> Result<Json<Response<PrinterCapability>>> {
    let printers = PrinterDevice::all().unwrap_or_default();
    let printer = printers.iter().find(|p| p.name() == name.0);

    if let Some(printer) = printer {
      let cap = PrintCapabilities::fetch(printer).map_err(InternalServerError)?;
      debug!("Printer {}: {:#?}", name.0, cap);

      let pcap = PrinterCapability {
        max_copies: cap.max_copies().map(|cp| cp.0),
        orientations: get_orientations(&cap),
        page_sizes: get_page_sizes(&cap),
      };

      Ok(Response::ok(pcap))
    } else {
      Ok(Response::err("No such printer"))
    }
  }

  /// 获取默认打印设置
  #[oai(
    path = "/settings",
    method = "get",
    operation_id = "getDefaultSettings"
  )]
  async fn get_default_settings(&self) -> Result<Json<Response<PrintSettings>>> {
    if let Some(filepath) = get_settings_filepath() {
      if let Ok(settings) = read_settings(filepath) {
        Ok(Response::ok(settings))
      } else {
        Ok(Response::err("No default settings"))
      }
    } else {
      Ok(Response::err("No default settings"))
    }
  }

  /// 打印 PDF 文件
  #[oai(path = "/print", method = "post", operation_id = "print")]
  async fn print(&self, payload: Json<PrintPayload>) -> Result<Json<Response<String>>> {
    let result = print_file(&payload);

    if let Err(e) = result {
      error!("Print error: {:#?}", e);
      Ok(Response::err(format!("Failed to print: {}", e.to_string())))
    } else {
      Ok(Response::ok("ok".to_string()))
    }
  }
}

fn get_orientations(cap: &PrintCapabilities) -> Option<Vec<Orientation>> {
  let mut oriens = Vec::new();

  for ori in cap.page_orientations() {
    if let Some(ori) = ori.as_predefined_name() {
      oriens.push(ori.into());
    }
  }

  if oriens.is_empty() {
    None
  } else {
    Some(oriens)
  }
}

fn get_page_sizes(cap: &PrintCapabilities) -> Option<Vec<PageSize>> {
  let sizes: Vec<_> = cap
    .page_media_sizes()
    .map(|pms| {
      let s = pms.size();
      PageSize {
        name: pms
          .display_name()
          // 安装 Gprinter GP-1134T 打印驱动发现纸张大小有“&#xEB;米”字样，不知道怎么来的
          .map(|s| s.replace("&#xEB;米", "毫米").to_string()),
        width: s.width_in_micron(),
        height: s.height_in_micron(),
      }
    })
    .collect();

  if sizes.is_empty() {
    None
  } else {
    Some(sizes)
  }
}

fn print_file(payload: &PrintPayload) -> anyhow::Result<()> {
  // 查找打印机
  let printers = PrinterDevice::all()?;
  let printer = printers
    .iter()
    .find(|p| p.name().replace("&#xEB;米", "毫米") == payload.settings.printer);

  if printer.is_none() {
    bail!("No such printer");
  }

  // 应用打印设置
  let printer = printer.unwrap();
  let cap = PrintCapabilities::fetch(printer)?;
  let mut builder = PrintTicketBuilder::new(printer)?;

  // 份数
  if let Some(copies) = payload.settings.copies {
    builder.merge(Copies(copies))?;
  }

  // 布局
  if let Some(ori) = &payload.settings.orientation {
    let predefined = Some(ori.into());
    let ori = cap
      .page_orientations()
      .find(|x| x.as_predefined_name() == predefined);

    if let Some(ori) = ori {
      builder.merge(ori)?;
    } else {
      bail!("No such orientation");
    }
  }

  // 纸张大小
  if let Some(page_size) = &payload.settings.page_size {
    let page = if let Some(name) = &page_size.name {
      let name = Some(name.as_str());
      cap.page_media_sizes().find(|x| x.display_name() == name)
    } else {
      cap.page_media_sizes().find(|x| {
        let size = x.size();
        size.width_in_micron() == page_size.width && size.height_in_micron() == page_size.height
      })
    };

    if let Some(page) = page {
      builder.merge(page)?;
    } else {
      bail!("No such page size");
    }
  }

  // 保存临时文件
  let mut file = NamedTempFile::new()?;
  file.write_all(&payload.file)?;

  // 打印
  let ticket = builder.build()?;
  let pdf = PdfiumPrinter::new(printer.clone());
  pdf.print(file.path(), ticket)?;

  Ok(())
}

fn get_settings_filepath() -> Option<PathBuf> {
  if let Some(dir) = ProjectDirs::from("com", "ubesthelp", env!("CARGO_PKG_NAME")) {
    let mut filepath: PathBuf = dir.config_local_dir().to_path_buf();
    filepath.push("default.json");
    Some(filepath)
  } else {
    None
  }
}

fn read_settings(filepath: PathBuf) -> anyhow::Result<PrintSettings> {
  let json = read_to_string(filepath)?;

  match PrintSettings::parse_from_json_string(&json) {
    Ok(settings) => Ok(settings),
    Err(e) => {
      error!("Failed to parse settings file: {:#?}", e);
      bail!("Failed to parse settings file: {:#?}", e);
    }
  }
}
