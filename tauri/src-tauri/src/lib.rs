mod commands;
use commands::{greet, login_data, pimsg, uimsg, zimsg, ZkState};
use girlboss::{Girlboss, Monitor};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::SystemTime;
use tauri::Manager;
use zknotes_server_lib::err_main;
use zknotes_server_lib::jobs::JobId;
use zknotes_server_lib::state::State;

// THIS IS THE ONE FOR ANDROID!

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let gb: Girlboss<JobId, Monitor> = Girlboss::new();
  let state = State {
    config: zknotes_server_lib::defcon(),
    girlboss: { Arc::new(RwLock::new(gb)) },
    jobcounter: { RwLock::new(0 as i64) },
  };

  tauri::Builder::default()
    .manage(ZkState {
      state: Arc::new(RwLock::new(state)),
    })
    .setup(|app| {
      // println!("dbpath: {:?}", dbpath);
      match app.state::<ZkState>().state.write() {
        Ok(mut state) => {
          // let datapath = app.path().data_dir().unwrap();
          let datapath = app.path().document_dir().unwrap();
          let mut dbpath = datapath.clone();
          dbpath.push("zknotes.db");
          let mut filepath = datapath.clone();
          filepath.push("files");
          let mut temppath = datapath.clone();
          temppath.push("temp");

          let mut logpath = app.path().document_dir().unwrap();
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

          zknotes_server_lib::sqldata::dbinit(
            state.config.orgauth_config.db.as_path(),
            state.config.orgauth_config.login_token_expiration_ms,
          )?;

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

          // spawn the web server in a separate thread.
          let _handler = thread::spawn(|| match err_main(Some(cc), Some(logpath)) {
            Err(e) => println!("error: {:?}", e),
            Ok(_) => (),
          });
        }
        Err(_) => (),
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      greet, zimsg, pimsg, uimsg, login_data
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
