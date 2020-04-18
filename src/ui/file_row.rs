use glib::subclass::prelude::*;
use glib::subclass::simple::{ClassStruct, InstanceStruct};
use glib::translate::{FromGlibPtrFull, ToGlib, ToGlibPtr};
use glib::types::StaticType;
use glib::{glib_object_wrapper, glib_wrapper, Cast, Object};
use std::path::Path;
use std::rc::Rc;

glib_wrapper! {
    pub struct FileRow(Object<
        InstanceStruct<imp::FileRow>,
        ClassStruct<imp::FileRow>,
        FileRowClass
    >);

    match fn {
        get_type => || imp::FileRow::get_type().to_glib(),
    }
}

impl FileRow {
    pub fn new() -> Self {
        Object::new(Self::static_type(), &[])
            .expect("failed to create FileRow")
            .downcast()
            .unwrap()
    }

    fn imp(&self) -> &imp::FileRow {
        imp::FileRow::from_instance(self)
    }

    pub fn set_path(&self, path: &Path) {
        self.imp().path.replace(Rc::from(path));
    }

    pub fn get_path(&self) -> Rc<Path> {
        self.imp().path.clone().into_inner()
    }
}

mod imp {
    use glib::subclass::prelude::*;
    use glib::subclass::simple::{ClassStruct, InstanceStruct};
    use glib::{glib_object_impl, glib_object_subclass, Object};
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;

    pub struct FileRow {
        pub path: RefCell<Rc<Path>>,
    }

    impl ObjectSubclass for FileRow {
        const NAME: &'static str = "FileRow";
        type ParentType = Object;
        type Instance = InstanceStruct<Self>;
        type Class = ClassStruct<Self>;

        glib_object_subclass!();

        fn new() -> Self {
            FileRow {
                path: RefCell::new(Rc::from(Path::new(""))),
            }
        }
    }

    impl ObjectImpl for FileRow {
        glib_object_impl!();
    }
}
