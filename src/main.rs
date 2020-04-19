use crate::sampler::TaskContext;
use crate::ui::{Progress, UiOpt, UiRes};
use anyhow::Result;
use std::thread;

mod res;
mod sampler;
mod ui;

fn main() -> Result<()> {
    gtk::init()?;
    gst::init()?;

    let _res_holder = res::load()?;
    let (task_sender, task_receiver) = crossbeam_channel::unbounded();
    let UiRes { progress_sender } = ui::init(UiOpt { task_sender });

    for _ in 0..num_cpus::get_physical() {
        let task_receiver = task_receiver.clone();
        let progress_sender = progress_sender.clone();
        thread::spawn(move || {
            let mut ctx = TaskContext {
                rng: rand::thread_rng(),
            };
            loop {
                let task = match task_receiver.recv() {
                    Ok(task) => task,
                    Err(_) => break,
                };
                let ref_idx = task.ref_idx;
                let result = sampler::run_task(&mut ctx, task, |p| {
                    let _ = progress_sender.send(Progress {
                        ref_idx,
                        progress: Some(p),
                    });
                });
                if let Err(e) = result {
                    eprintln!("error: {:?}", e);
                    let _ = progress_sender.send(Progress {
                        ref_idx,
                        progress: None,
                    });
                }
            }
        });
    }

    gtk::main();

    Ok(())
}
