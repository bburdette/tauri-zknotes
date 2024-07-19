mod commands;
use commands::{greet, pimsg, uimsg, zimsg, ZkState};
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(ZkState {
      config: Mutex::new(zknotes_server_lib::defcon()),
      uid: Mutex::new(None),
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
        }
        Err(_) => (),
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![greet, zimsg, pimsg, uimsg])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
