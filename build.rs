#[cfg(windows)]
extern crate winres;

fn main() {
  if cfg!(windows) {
    let res = winres::WindowsResource::new();
    res.compile().unwrap();
  }
}
