use log::debug;
use poem::{error::InternalServerError, Result};
use poem_openapi::{
  param::Path,
  payload::Json,
  types::{ParseFromJSON, ToJSON},
  Enum, Object, OpenApi, Tags,
};
use winprint::{
  printer::PrinterDevice,
  ticket::{
    FeatureOptionPack, FeatureOptionPackWithPredefined, PredefinedPageOrientation,
    PrintCapabilities,
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
#[derive(Enum)]
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

/// 纸张大小
#[derive(Object)]
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
#[derive(Default, Object)]
#[oai(skip_serializing_if_is_none)]
struct PrinterCapability {
  /// 最大打印份数
  max_copies: Option<u16>,
  /// 布局
  orientations: Option<Vec<Orientation>>,
  /// 纸张大小
  page_sizes: Option<Vec<PageSize>>,
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
  #[oai(path = "/printers", method = "get")]
  async fn get_printers(&self) -> Json<Response<Vec<String>>> {
    let printers = PrinterDevice::all().unwrap_or_default();
    Response::ok(printers.iter().map(|p| p.name().to_string()).collect())
  }

  /// 获取指定打印机能力。
  #[oai(path = "/printers/:name", method = "get")]
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
        name: pms.display_name().map(|s| s.to_string()),
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
