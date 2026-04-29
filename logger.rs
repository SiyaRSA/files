use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

static LOGGER: Mutex<Option<File>> = Mutex::new(None);


pub fn init_logger() {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("C:\\temp\\rws_logger.log");

    /*FILE_LOG.lock().unwrap() = Some(file);*/

    match file {
        Ok(f) => {
            *LOGGER.lock().unwrap() = Some(f);
            log("logger initialized");
        }
        Err(e) => {
            eprintln!("Failed to init logger: {}", e);
        }
    }
}

pub fn log(msg: &str) {
    if let Ok(mut guard) = LOGGER.lock() {
        if let Some(file) = guard.as_mut() {
            let _ = writeln!(file, "{}", msg);
        }
    }
}