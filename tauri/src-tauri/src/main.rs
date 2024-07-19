#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// use log::{error, info};
// use serde;
// use serde_json;
// use serde_json::Value;
use std::sync::Mutex;
// use std::thread;
// use tauri::State;
// use zknotes_server_lib::err_main;
// use zknotes_server_lib::orgauth::data::WhatMessage;
// use zknotes_server_lib::orgauth::endpoints::{Callbacks, Tokener, UuidTokener};
// use zknotes_server_lib::zkprotocol::messages::{PublicMessage, ServerResponse, UserMessage};
mod commands;
use commands::{greet, pimsg, uimsg, zimsg, ZkState};
use std::error::Error;

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
fn main() {
  // spawn the web server in a separate thread.
  // let handler = thread::spawn(|| {
  //   println!("meh here");
  //   match err_main() {
  //     Err(e) => error!("error: {:?}", e),
  //     Ok(_) => (),
  //   }
  // });

  let res: Result<zknotes_server_lib::config::Config, Box<dyn Error>> = (|| {
    let config = zknotes_server_lib::load_config("zknotes-tauri-dev.toml")?;
    let ret = zknotes_server_lib::sqldata::dbinit(
      config.orgauth_config.db.as_path(),
      config.orgauth_config.login_token_expiration_ms,
    );
    println!("dbinit ret: {:?}", ret);
    // verify/create file directories.
    if config.createdirs {
      if !std::path::Path::exists(&config.file_tmp_path) {
        std::fs::create_dir_all(&config.file_tmp_path)?;
      }
      if !std::path::Path::exists(&config.file_path) {
        std::fs::create_dir_all(&config.file_path)?;
      }
    }

    Ok(config)
  })();

  match res {
    Ok(config) => {
      tauri::Builder::default()
        .manage(ZkState {
          config: Mutex::new(config),
          uid: None.into(),
        })
        .invoke_handler(tauri::generate_handler![greet, zimsg, pimsg, uimsg])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    }
    Err(e) => {
      println!("bad config: {}", e);
      panic!("bye");
    }
  }

  // #[cfg(desktop)]
  // app_lib::run();
}
