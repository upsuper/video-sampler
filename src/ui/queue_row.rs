use glib::subclass::prelude::*;
use glib::subclass::simple::{ClassStruct, InstanceStruct};
use glib::translate::{FromGlibPtrFull, ToGlib, ToGlibPtr};
use glib::types::StaticType;
use glib::{glib_object_wrapper, glib_wrapper, Cast, Object};

glib_wrapper! {
    pub struct QueueRow(Object<
        InstanceStruct<imp::QueueRow>,
        ClassStruct<imp::QueueRow>,
        QueueRowClass
    >);

    match fn {
        get_type => || imp::QueueRow::get_type().to_glib(),
    }
}

impl QueueRow {
    pub fn new() -> Self {
        Object::new(Self::static_type(), &[])
            .expect("failed to create QueueRow")
            .downcast()
            .unwrap()
    }
}

mod imp {
    use glib::subclass::prelude::*;
    use glib::subclass::simple::{ClassStruct, InstanceStruct};
    use glib::subclass::Property;
    use glib::{glib_object_impl, glib_object_subclass, ToValue};
    use glib::{Object, ParamFlags, ParamSpec, Value};
    use std::cell::{Cell, RefCell};

    pub struct QueueRow {
        name: RefCell<Box<str>>,
        progress: Cell<f64>,
    }

    static PROPERTIES: &[Property] = &[
        Property("name", |name| {
            ParamSpec::string(name, "Name", "Name", None, ParamFlags::READWRITE)
        }),
        Property("progress", |name| {
            ParamSpec::double(
                name,
                "Progress",
                "Progress",
                0.,
                1.,
                0.,
                ParamFlags::READWRITE,
            )
        }),
    ];

    impl ObjectSubclass for QueueRow {
        const NAME: &'static str = "QueueRow";
        type ParentType = Object;
        type Instance = InstanceStruct<Self>;
        type Class = ClassStruct<Self>;

        glib_object_subclass!();

        fn class_init(klass: &mut Self::Class) {
            klass.install_properties(&PROPERTIES);
        }

        fn new() -> Self {
            Self {
                name: RefCell::new(String::new().into_boxed_str()),
                progress: Cell::new(0.),
            }
        }
    }

    impl ObjectImpl for QueueRow {
        glib_object_impl!();

        fn set_property(&self, _: &Object, id: usize, value: &Value) {
            match &PROPERTIES[id] {
                Property("name", ..) => {
                    let name = value.get::<String>().expect("expected string");
                    let name = name.unwrap_or_default();
                    self.name.replace(name.into_boxed_str());
                }
                Property("progress", ..) => {
                    let value = value.get().expect("expected float").unwrap_or_default();
                    self.progress.replace(value);
                }
                _ => unreachable!("unknown property"),
            }
        }

        fn get_property(&self, _: &Object, id: usize) -> Result<Value, ()> {
            match &PROPERTIES[id] {
                Property("name", ..) => Ok(self.name.borrow().to_value()),
                Property("progress", ..) => Ok(self.progress.get().to_value()),
                _ => unreachable!("unknown property"),
            }
        }
    }
}
