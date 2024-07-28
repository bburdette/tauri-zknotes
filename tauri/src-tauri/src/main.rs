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
use commands::{fileresp, greet, pimsg, uimsg, zimsg, ZkState};
use std::error::Error;
use tauri::{http, utils::mime_type, Manager};

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
        // .register_asynchronous_uri_scheme_protocol("zkfile", fileresp)
        .register_asynchronous_uri_scheme_protocol("zkfile", |app, request, responder| {
          println!("fileresp");
          fileresp(app.state(), request, responder);
        })
        // .register_asynchronous_uri_scheme_protocol("zkfile", |app, request, responder| {
        //   // app.state()
        //   // app = 1;
        //   // request = 5;
        //   // responder = 7;
        //   println!("uri scheme req: {:?}", request);
        //   responder.respond(
        //     http::Response::builder()
        //       .status(http::StatusCode::BAD_REQUEST)
        //       .header(
        //         http::header::CONTENT_TYPE,
        //         mime_type::MimeType::Txt.to_string().as_str(),
        //       )
        //       .body("failed to read file".as_bytes().to_vec())
        //       .unwrap(),
        //   );
        // })
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

/*
tauri::Builder::default()
  .register_asynchronous_uri_scheme_protocol("app-files", |_app, request, responder| {
    // skip leading `/`
    let path = request.uri().path()[1..].to_string();
    std::thread::spawn(move || {
      if let Ok(data) = std::fs::read(path) {
        responder.respond(
          http::Response::builder()
            .body(data)
            .unwrap()
        );
      } else {
        responder.respond(
          http::Response::builder()
            .status(http::StatusCode::BAD_REQUEST)
            .header(http::header::CONTENT_TYPE, mime::TEXT_PLAIN.essence_str())
            .body("failed to read file".as_bytes().to_vec())
            .unwrap()
        );
    }
  });
  });
  */
