mod commands;
use commands::{greet, pimsg, uimsg, zimsg, ZkState};
use std::sync::Mutex;
use std::thread;
use std::time::SystemTime;
use tauri::Manager;
use zknotes_server_lib::err_main;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // spawn the web server in a separate thread.
  tauri::Builder::default()
    .manage(ZkState {
      config: Mutex::new(zknotes_server_lib::defcon()),
      uid: Mutex::new(None),
    })
    .setup(|app| {
      // println!("dbpath: {:?}", dbpath);
      match app.state::<ZkState>().config.lock() {
        Ok(mut config) => {
          // let datapath = app.path().data_dir().unwrap();
          let datapath = app.path().document_dir().unwrap();
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
    .invoke_handler(tauri::generate_handler![greet, zimsg, pimsg, uimsg])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
