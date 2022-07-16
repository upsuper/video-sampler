use anyhow::{Context, Result};
use glib::Cast;
use gst::prelude::*;
use gst::{
    Bus, Caps, ClockTime, Element, ElementFactory, MessageView, Object, Pipeline, SeekFlags, State,
};
use gst_app::AppSink;
use png::{BitDepth, ColorType};
use rand::prelude::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;

pub struct TaskContext {
    pub rng: ThreadRng,
}

pub struct Task {
    pub prefix: Arc<str>,
    pub height: u32,
    pub samples: u32,
    pub target: Arc<Path>,
    pub index: u32,
    pub source: Box<Path>,
    /// Task reference for sending progress.
    pub ref_idx: u32,
}

pub fn run_task<P>(ctx: &mut TaskContext, task: Task, report_progress: P) -> Result<()>
where
    P: Fn(f64),
{
    let pipeline = Pipeline::new(None);
    let src = ElementFactory::make("filesrc", None)?;
    src.set_property("location", &task.source.to_str().unwrap())?;
    let sink = ElementFactory::make("appsink", None)?;
    let sink = sink.dynamic_cast::<AppSink>().unwrap();
    sink.set_max_buffers(1);
    let decodebin = ElementFactory::make("decodebin", None)?;
    let convert = ElementFactory::make("videoconvert", None)?;
    let scale = ElementFactory::make("videoscale", None)?;

    pipeline.add_many(&[&src, &decodebin, &convert, &scale, sink.as_ref()])?;
    src.link(&decodebin)?;
    Element::link_many(&[&convert, &scale, sink.as_ref()])?;

    // Before we change the state, ensure we reset it when we return.
    // This is important when we return from error path.
    let _auto_reset_pipeline = AutoStateReset(pipeline.clone());
    pipeline
        .set_state(State::Paused)
        .context("failed to set pipeline state to paused")?;

    let bus = pipeline.get_bus().unwrap();
    wait_for_state_change_to(&bus, decodebin.as_ref(), State::Paused);

    // Setup the sink to accept the data we want.
    let (orig_width, orig_height) = decodebin
        .get_src_pads()
        .into_iter()
        .filter_map(|pad| {
            let caps = pad.get_current_caps()?;
            let s = caps.get_structure(0)?;
            if !s.get_name().starts_with("video/") {
                return None;
            }
            let width = s.get_some::<i32>("width").ok()?;
            let height = s.get_some::<i32>("height").ok()?;
            Some((width, height))
        })
        .next()
        .context("no video dimension found")?;
    let height = task.height as i32;
    let width = orig_width * height / orig_height;
    let caps = Caps::builder("video/x-raw")
        .field("format", &"RGB")
        .field("width", &width)
        .field("height", &height)
        .build();
    sink.set_caps(Some(&caps));

    // Connect the video handling side of pipeline on to decodebin.
    decodebin
        .link(&convert)
        .context("failed to link decodebin to videoconvert")?;

    // Query the duration of the video.
    let duration = pipeline
        .query_duration::<ClockTime>()
        .context("failed to get duration")?;
    // Generate sample offsets.
    let mut samples = (0..task.samples)
        .map(|_| ClockTime::from_nseconds(ctx.rng.gen()) % duration)
        .collect::<Vec<_>>();
    samples.sort();

    wait_for_state_change_to(&bus, pipeline.as_ref(), State::Paused);

    for (i, seek_pos) in samples.into_iter().enumerate() {
        // Seek to the given place and get the data buffer.
        pipeline.seek_simple(SeekFlags::FLUSH | SeekFlags::ACCURATE, seek_pos)?;
        let sample = sink.pull_preroll()?;
        let buffer = sample.get_buffer().context("failed to get buffer")?;
        let buffer = buffer.map_readable()?;
        let buffer = buffer.as_slice();

        // Output to the image file.
        let file_name = format!(
            "{}-{}-{}-{}-{:03}.png",
            task.prefix,
            task.index,
            seek_pos.minutes().unwrap(),
            seek_pos.seconds().unwrap() % 60,
            seek_pos.mseconds().unwrap() % 1000,
        );
        let output_path = task.target.join(file_name);
        let output = File::create(&output_path).context("failed to create output file")?;
        let output = BufWriter::new(output);
        let mut encoder = png::Encoder::new(output, width as u32, height as u32);
        encoder.set_color(ColorType::Rgb);
        encoder.set_depth(BitDepth::Eight);
        encoder
            .write_header()
            .context("failed to write header")?
            .write_image_data(buffer)
            .context("failed to write image data")?;

        report_progress((i + 1) as f64 / task.samples as f64);
    }

    Ok(())
}

struct AutoStateReset<T: IsA<Element>>(T);

impl<T: IsA<Element>> Drop for AutoStateReset<T> {
    fn drop(&mut self) {
        let _ = self.0.set_state(State::Null);
    }
}

fn wait_for_state_change_to(bus: &Bus, src: &Object, state: State) {
    wait_for_message_from(bus, src, |view| match view {
        MessageView::StateChanged(view) => view.get_current() == state,
        _ => false,
    });
}

fn wait_for_message_from<P: Fn(MessageView) -> bool>(bus: &Bus, src: &Object, predicate: P) {
    for msg in bus.iter_timed(ClockTime::none()) {
        if msg.get_src().as_ref() == Some(src) && predicate(msg.view()) {
            break;
        }
    }
}
