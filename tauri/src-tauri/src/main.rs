#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::{get_platform, greet, login_data, pimsg, timsg, uimsg, zimsg, ZkState};
use girlboss::{Girlboss, Monitor};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::SystemTime;
use tauri::Manager;
use time;
use zknotes_server_lib::err_main;
use zknotes_server_lib::jobs::JobId;
use zknotes_server_lib::sqldata::Server;
use zknotes_server_lib::state::State;

// THIS IS THE ONE FOR DESKTOP!

fn main() {
  let gb: Girlboss<JobId, Monitor> = Girlboss::new();
  let state = State {
    config: zknotes_server_lib::defcon(),
    girlboss: { Arc::new(RwLock::new(gb)) },
    jobcounter: { RwLock::new(0 as i64) },
    // server placeholder value
    server: Server {
      id: 0,
      uuid: "".to_string(),
    },
  };

  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .manage(ZkState {
      state: Arc::new(RwLock::new(state)),
    })
    .setup(|app| {
      match app.state::<ZkState>().state.write() {
        Ok(mut state) => {
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

          state.config.orgauth_config.db = dbpath;
          state.config.createdirs = true;
          state.config.file_path = filepath;
          state.config.file_tmp_path = temppath;
          state.config.tauri_mode = true;
          state.config.orgauth_config.open_registration = true;

          // load real server value
          let server = zknotes_server_lib::sqldata::dbinit(
            state.config.orgauth_config.db.as_path(),
            state.config.orgauth_config.login_token_expiration_ms,
          )?;

          state.server = server;

          // verify/create file directories.
          if state.config.createdirs {
            if !std::path::Path::exists(&state.config.file_tmp_path) {
              std::fs::create_dir_all(&state.config.file_tmp_path)?
            }
            if !std::path::Path::exists(&state.config.file_path) {
              std::fs::create_dir_all(&state.config.file_path)?
            }
          }

          let cc = state.config.clone();

          let _handler = thread::spawn(|| match err_main(Some(cc), Some(logpath)) {
            Err(e) => println!("error: {:?}", e),
            Ok(_) => (),
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
    .invoke_handler(tauri::generate_handler![
      greet,
      zimsg,
      pimsg,
      uimsg,
      login_data,
      timsg,
      get_platform
    ])
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
