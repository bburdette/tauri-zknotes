#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::thread;
use std::{sync::Mutex, time::SystemTime};
use zknotes_server_lib::err_main;
mod commands;
use commands::{fileresp, greet, pimsg, uimsg, zimsg, ZkState};
use std::error::Error;
use tauri::{http, utils::mime_type, Manager};
use time;

const DATE_FORMAT_STR: &'static str = "%Y-%m-%dT%H:%M:%S";

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
fn main() {
  // let res: Result<zknotes_server_lib::config::Config, Box<dyn Error>> = (|| {
  //   let config = zknotes_server_lib::load_config("zknotes-tauri-dev.toml")?;
  //   let ret = zknotes_server_lib::sqldata::dbinit(
  //     config.orgauth_config.db.as_path(),
  //     config.orgauth_config.login_token_expiration_ms,
  //   );
  //   println!("dbinit ret: {:?}", ret);
  //   // verify/create file directories.
  //   if config.createdirs {
  //     if !std::path::Path::exists(&config.file_tmp_path) {
  //       std::fs::create_dir_all(&config.file_tmp_path)?;
  //     }
  //     if !std::path::Path::exists(&config.file_path) {
  //       std::fs::create_dir_all(&config.file_path)?;
  //     }
  //   }

  //   Ok(config)
  // })();

  tauri::Builder::default()
    .manage(ZkState {
      config: Mutex::new(zknotes_server_lib::defcon()),
      uid: None.into(),
    })
    .setup(|app| {
      // println!("dbpath: {:?}", dbpath);
      match app.state::<ZkState>().config.lock() {
        Ok(mut config) => {
          let datapath = app.path().data_dir().unwrap();
          let mut dbpath = datapath.clone();
          dbpath.push("zknotes.db");
          let mut filepath = datapath.clone();
          filepath.push("files");
          let mut temppath = datapath.clone();
          temppath.push("temp");

          let mut logpath = app.path().home_dir().unwrap();
          let dt: time::OffsetDateTime = SystemTime::now().into();
          let f = time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]");

          match f.map(|dtf| dt.format(&dtf)) {
            Ok(Ok(dtf)) => logpath.push(format!("{}.zknotes.log", dtf)),
            _ => logpath.push("zknotes.log"),
          };

          println!("logpath {:?}", logpath);

          config.orgauth_config.db = dbpath;
          config.createdirs = true;
          config.file_path = filepath;
          config.file_tmp_path = temppath;
          config.orgauth_config.open_registration = true;

          zknotes_server_lib::sqldata::dbinit(
            config.orgauth_config.db.as_path(),
            config.orgauth_config.login_token_expiration_ms,
          )?;

          // verify/create file directories.
          if config.createdirs {
            if !std::path::Path::exists(&config.file_tmp_path) {
              std::fs::create_dir_all(&config.file_tmp_path)?
            }
            if !std::path::Path::exists(&config.file_path) {
              std::fs::create_dir_all(&config.file_path)?
            }
          }

          let cc = config.clone();

          let _handler = thread::spawn(|| {
            println!("meh here");
            match err_main(Some(cc), Some(logpath)) {
              Err(e) => println!("error: {:?}", e),
              Ok(_) => (),
            }
          });
        }
        Err(_) => (),
      }
      Ok(())
    })
    // .register_asynchronous_uri_scheme_protocol("zkfile", |app, request, responder| {
    //   println!("fileresp");
    //   fileresp(app.state::<ZkState>(), request, responder);
    // })
    .invoke_handler(tauri::generate_handler![greet, zimsg, pimsg, uimsg])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

/*

// this is a promising approach, but there's a bug in webkit that prevents it from working,
// apparently.

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
