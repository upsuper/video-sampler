use self::file_row::FileRow;
use self::queue_row::QueueRow;
use crate::resource_path;
use crate::sampler::Task;
use gdk::DragAction;
use gdk_pixbuf::Pixbuf;
use gio::prelude::*;
use glib::{GString, MainContext, PRIORITY_DEFAULT};
use gtk::prelude::*;
use gtk::{
    Align, Button, DestDefaults, Entry, FileChooserButton, Label, ListBox, ProgressBar,
    TargetEntry, TargetFlags, Window,
};
use pango::EllipsizeMode;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use url::Url;

mod file_row;
mod queue_row;

pub struct Progress {
    /// Task reference, see `Task::ref_idx`.
    pub ref_idx: u32,
    pub progress: Option<f64>,
}

pub struct UiOpt {
    pub(crate) task_sender: crossbeam_channel::Sender<Task>,
}

pub struct UiRes {
    pub(crate) progress_sender: glib::Sender<Progress>,
}

pub fn init(opt: UiOpt) -> UiRes {
    let builder = gtk::Builder::new_from_resource(resource_path!("/main.glade"));
    let window: Window = builder.get_object("window_main").unwrap();
    let entry_prefix: Entry = builder.get_object("entry_prefix").unwrap();
    let entry_height: Entry = builder.get_object("entry_height").unwrap();
    let entry_samples: Entry = builder.get_object("entry_samples").unwrap();
    let file_target: FileChooserButton = builder.get_object("file_target").unwrap();
    let list_files: ListBox = builder.get_object("list_files").unwrap();
    let list_queue: ListBox = builder.get_object("list_queue").unwrap();
    let button_clear: Button = builder.get_object("button_clear").unwrap();
    let button_queue: Button = builder.get_object("button_queue").unwrap();

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

    let task_sender = opt.task_sender;
    button_queue.connect_clicked({
        let entry_prefix = entry_prefix.downgrade();
        let entry_height = entry_height.downgrade();
        let entry_samples = entry_samples.downgrade();
        let file_target = file_target.downgrade();
        let files = files.downgrade();
        let queue = queue.downgrade();
        move |_| {
            let entry_prefix = entry_prefix.upgrade().unwrap();
            let entry_height = entry_height.upgrade().unwrap();
            let entry_samples = entry_samples.upgrade().unwrap();
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
            let height = match parse_from_entry(&entry_height) {
                Some(n) => n,
                None => return,
            };
            // Get sample number per video
            let samples = match parse_from_entry(&entry_samples) {
                Some(n) => n,
                None => return,
            };
            // Get target path
            let target = match file_target.get_uri().as_ref().and_then(file_uri_to_path) {
                Some(path) => Arc::<Path>::from(path.as_path()),
                None => return,
            };
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
                        target: target.clone(),
                        index: i,
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

fn parse_from_entry<T: FromStr>(entry: &Entry) -> Option<T> {
    entry.get_text().and_then(|s| s.as_str().parse().ok())
}
