use anyhow::Result;
use gio::{resources_register, resources_unregister, Resource};
use glib::Bytes;

#[macro_export]
macro_rules! resource_path {
    ($path:literal) => {
        concat!("/org/upsuper/video-sampler", $path)
    };
}

static RESOURCE_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/res.gresource"));

pub fn load() -> Result<ResourceHolder> {
    let data = Bytes::from_static(RESOURCE_BINARY);
    let res = Resource::from_data(&data)?;
    Ok(ResourceHolder::new(res))
}

pub struct ResourceHolder(Resource);

impl ResourceHolder {
    fn new(res: Resource) -> Self {
        resources_register(&res);
        ResourceHolder(res)
    }
}

impl Drop for ResourceHolder {
    fn drop(&mut self) {
        resources_unregister(&self.0);
    }
}
