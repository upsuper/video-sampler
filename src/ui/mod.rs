use self::file_row::FileRow;
use self::queue_row::QueueRow;
use crate::sampler::Task;
use crate::{resource_path, Config};
use gdk::DragAction;
use gdk_pixbuf::Pixbuf;
use gio::prelude::*;
use glib::{GString, MainContext, PRIORITY_DEFAULT};
use gtk::prelude::*;
use gtk::{
    Adjustment, Align, Button, DestDefaults, Entry, FileChooserButton, Label, ListBox, ProgressBar,
    TargetEntry, TargetFlags, Window,
};
use pango::EllipsizeMode;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use url::Url;

mod file_row;
mod queue_row;

pub struct Progress {
    /// Task reference, see `Task::ref_idx`.
    pub ref_idx: u32,
    pub progress: Option<f64>,
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct DefaultConfig {
    height: u32,
    samples: u32,
    target: Option<PathBuf>,
}

impl Default for DefaultConfig {
    fn default() -> Self {
        DefaultConfig {
            height: 360,
            samples: 5,
            target: None,
        }
    }
}

pub struct UiOpt {
    pub config: Rc<RefCell<Config>>,
    pub task_sender: crossbeam_channel::Sender<Task>,
}

pub struct UiRes {
    pub progress_sender: glib::Sender<Progress>,
}

pub fn init(opt: UiOpt) -> UiRes {
    let UiOpt {
        config,
        task_sender,
    } = opt;

    let builder = gtk::Builder::new_from_resource(resource_path!("/main.glade"));
    let window: Window = builder.get_object("window_main").unwrap();
    let adjust_samples: Adjustment = builder.get_object("adjust_samples").unwrap();
    let entry_prefix: Entry = builder.get_object("entry_prefix").unwrap();
    let entry_height: Entry = builder.get_object("entry_height").unwrap();
    let file_target: FileChooserButton = builder.get_object("file_target").unwrap();
    let list_files: ListBox = builder.get_object("list_files").unwrap();
    let list_queue: ListBox = builder.get_object("list_queue").unwrap();
    let button_clear: Button = builder.get_object("button_clear").unwrap();
    let button_queue: Button = builder.get_object("button_queue").unwrap();

    // Set the default value from the config
    let config_ref = config.borrow();
    let default_config = &config_ref.default;
    entry_height.set_text(&default_config.height.to_string());
    adjust_samples.set_value(default_config.samples as _);
    let target_uri = default_config
        .target
        .as_ref()
        .and_then(|path| Url::from_directory_path(path).ok())
        .map(|url| url.into_string());
    if let Some(uri) = target_uri {
        file_target.set_uri(&uri);
    }

    let icon = Pixbuf::new_from_resource(resource_path!("/icon-64.png")).unwrap();
    window.set_icon(Some(&icon));
    window.set_title("Video Sampler");
    window.connect_hide(|_| gtk::main_quit());
    window.show_all();

    let files = gio::ListStore::new(FileRow::static_type());
    list_files.bind_model(Some(&files), |item| {
        let path = item.downcast_ref::<FileRow>().unwrap().get_path();
        let label = Label::new(file_name_str(&path));
        label.set_halign(Align::Start);
        label.set_ellipsize(EllipsizeMode::Middle);
        label.show();
        label.upcast()
    });
    list_files.drag_dest_set(
        DestDefaults::ALL,
        &[TargetEntry::new("text/uri-list", TargetFlags::OTHER_APP, 0)],
        DragAction::COPY,
    );
    list_files.connect_drag_data_received({
        let files = files.downgrade();
        move |_, _, _, _, selection, _, _| {
            let files = files.upgrade().unwrap();
            for uri in selection.get_uris().iter() {
                let path = match file_uri_to_path(uri) {
                    Some(path) => path,
                    None => continue,
                };
                if file_name_str(&path).is_none() {
                    continue;
                }
                let row = FileRow::new();
                row.set_path(&path);
                files.insert_sorted(&row, |a, b| {
                    let a = a.downcast_ref::<FileRow>().unwrap().get_path();
                    let b = b.downcast_ref::<FileRow>().unwrap().get_path();
                    let a = file_name_str(&a);
                    let b = file_name_str(&b);
                    a.cmp(&b)
                });
            }
        }
    });

    let queue = gio::ListStore::new(QueueRow::static_type());
    list_queue.bind_model(Some(&queue), |item| {
        let builder = gtk::Builder::new_from_resource(resource_path!("/queue_row.glade"));
        let progress: ProgressBar = builder.get_object("progress").unwrap();
        let label_name: Label = builder.get_object("label_name").unwrap();
        item.bind_property("name", &label_name, "label")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .build();
        item.bind_property("progress", &progress, "fraction")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .build();
        builder.get_object("box_row").unwrap()
    });

    button_clear.connect_clicked({
        let files = files.downgrade();
        move |_| files.upgrade().unwrap().remove_all()
    });

    button_queue.connect_clicked({
        let config = Rc::downgrade(&config);
        let adjust_samples = adjust_samples.downgrade();
        let entry_prefix = entry_prefix.downgrade();
        let entry_height = entry_height.downgrade();
        let file_target = file_target.downgrade();
        let files = files.downgrade();
        let queue = queue.downgrade();
        move |_| {
            let config = config.upgrade().unwrap();
            let adjust_samples = adjust_samples.upgrade().unwrap();
            let entry_prefix = entry_prefix.upgrade().unwrap();
            let entry_height = entry_height.upgrade().unwrap();
            let file_target = file_target.upgrade().unwrap();
            let files = files.upgrade().unwrap();
            let queue = queue.upgrade().unwrap();
            // Get prefix
            let prefix = entry_prefix.get_text();
            let prefix = prefix.as_ref().map(GString::as_str);
            let prefix = Arc::<str>::from(prefix.unwrap_or(""));
            if prefix.is_empty() {
                return;
            }
            entry_prefix.set_text("");
            // Get sample height
            let height = entry_height
                .get_text()
                .and_then(|s| s.as_str().parse::<u32>().ok());
            let height = match height {
                Some(n) => n,
                None => return,
            };
            // Get sample number per video
            let samples = adjust_samples.get_value() as _;
            // Get target path
            let target = match file_target.get_uri().as_ref().and_then(file_uri_to_path) {
                Some(path) => path,
                None => return,
            };
            let target_arc = Arc::<Path>::from(target.as_path());
            let ref_base = queue.get_n_items();
            for i in 0..files.get_n_items() {
                let file: FileRow = files.get_object(i).unwrap().downcast().unwrap();
                let path = &*file.get_path();
                let source = Box::from(path);
                task_sender
                    .send(Task {
                        prefix: prefix.clone(),
                        height,
                        samples,
                        target: target_arc.clone(),
                        index: i + 1,
                        source,
                        ref_idx: ref_base + i,
                    })
                    .unwrap();
                let name = file_name_str(path);
                let queue_row = QueueRow::new();
                queue_row.set_property("name", &name).unwrap();
                queue_row.set_property("progress", &0.).unwrap();
                queue.append(&queue_row);
            }
            files.remove_all();
            // Save the config to default
            let mut config_ref = config.borrow_mut();
            let default_config = &mut config_ref.default;
            default_config.height = height;
            default_config.samples = samples;
            default_config.target = Some(target);
        }
    });

    let (progress_sender, progress_receiver) = MainContext::channel(PRIORITY_DEFAULT);
    progress_receiver.attach(None, {
        let queue = queue.downgrade();
        move |progress: Progress| {
            let queue = queue.upgrade().unwrap();
            let row = queue.get_object(progress.ref_idx).unwrap();
            row.set_property("progress", &progress.progress.unwrap_or_default())
                .unwrap();
            glib::Continue(true)
        }
    });

    UiRes { progress_sender }
}

fn file_uri_to_path(uri: &GString) -> Option<PathBuf> {
    let url = Url::parse(uri).ok()?;
    if url.scheme() == "file" {
        url.to_file_path().ok()
    } else {
        None
    }
}

fn file_name_str(path: &Path) -> Option<&str> {
    path.file_name().and_then(OsStr::to_str)
}
