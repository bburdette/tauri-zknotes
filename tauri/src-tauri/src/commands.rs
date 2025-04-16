use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tauri::utils::platform::Target;
use tauri::{http, utils::mime_type};
use tauri::{State, UriSchemeResponder};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use zknotes_server_lib::error as zkerr;
use zknotes_server_lib::orgauth::data::{LoginData, UserId, UserRequest};
use zknotes_server_lib::orgauth::dbfun;
use zknotes_server_lib::orgauth::endpoints::{Callbacks, UuidTokener};
use zknotes_server_lib::rusqlite::Connection;
use zknotes_server_lib::sqldata::{get_single_value, set_single_value};
use zknotes_server_lib::zkprotocol::private::{
  PrivateClosureReply, PrivateClosureRequest, PrivateError, PrivateReply, PrivateRequest,
};
use zknotes_server_lib::zkprotocol::public::{PublicError, PublicReply, PublicRequest};
use zknotes_server_lib::zkprotocol::tauri::{self as zt, TauriReply, UploadedFiles};
use zknotes_server_lib::{sqldata, UserResponse};

pub struct ZkState {
  pub state: Arc<RwLock<zknotes_server_lib::state::State>>,
}

#[tauri::command]
pub fn greet(name: &str) -> String {
  println!("greeet");
  format!("Hello, {}!", name)
}

#[tauri::command]
pub fn login_data(state: State<ZkState>) -> Result<Option<LoginData>, String> {
  let st = state.state.read().unwrap();
  let conn = match sqldata::connection_open(st.config.orgauth_config.db.as_path()) {
    Ok(c) => c,
    Err(e) => {
      return Err(e.to_string());
    }
  };

  get_tauri_login_data(&conn, &mut sqldata::zknotes_callbacks()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_platform() -> Result<Target, String> {
  Ok(Target::current())
}

pub fn get_tauri_uid(conn: &Connection) -> Result<Option<UserId>, zkerr::Error> {
  get_single_value(&conn, "last_login").and_then(|x| {
    Ok(x.and_then(|s| {
      serde_json::from_str::<i64>(s.as_str())
        .ok()
        .map(|x| UserId::Uid(x))
    }))
  })
}

pub fn get_tauri_login_data(
  conn: &Connection,
  callbacks: &mut Callbacks,
) -> Result<Option<LoginData>, zkerr::Error> {
  let uid = match get_tauri_uid(&conn)? {
    Some(uid) => uid,
    None => return Ok(None),
  };
  let mut ld = dbfun::login_data(&conn, uid)?;
  let data = (callbacks.extra_login_data)(&conn, ld.userid)?;
  ld.data = data.map(|x| x.to_string());
  Ok(Some(ld))
}

#[derive(Serialize, Deserialize)]
pub struct PrivateTimedData {
  utcmillis: u128,
  data: PrivateClosureReply,
}
#[derive(Serialize, Deserialize)]
pub struct PublicTimedData {
  utcmillis: u128,
  data: PublicReply,
}

pub fn fileresp(
  state: State<ZkState>,
  request: tauri::http::Request<Vec<u8>>,
  usr: UriSchemeResponder,
) {
  match fileresp_helper(state, request, usr) {
    Ok(()) => (),
    Err((usr, e)) => {
      usr.respond(
        http::Response::builder()
          .status(http::StatusCode::INTERNAL_SERVER_ERROR)
          .body(format!("{:?}", e).as_bytes().to_vec())
          .unwrap(),
      );
    }
  }
}

pub fn fileresp_helper(
  state: State<ZkState>,
  request: tauri::http::Request<Vec<u8>>,
  usr: UriSchemeResponder,
) -> Result<(), (UriSchemeResponder, zkerr::Error)> {
  let conn = match sqldata::connection_open(
    state
      .state
      .read()
      .unwrap()
      .config
      .orgauth_config
      .db
      .as_path(),
  ) {
    Ok(c) => c,
    Err(e) => {
      return Err((usr, e));
    }
  };

  let config_clone = state.state.read().unwrap().config.clone();

  let uid = match get_tauri_uid(&conn) {
    Ok(ld) => ld,
    Err(e) => return Err((usr, e)),
  };

  let noteid = match request
    .uri()
    .path_and_query()
    .map(|pnq| pnq.path())
    .and_then(|p| p.split("/").nth(1))
  {
    Some(noteid) => noteid,
    None => {
      usr.respond(
        http::Response::builder()
          .status(http::StatusCode::BAD_REQUEST)
          .body("file id required: /file/<id>".as_bytes().to_vec())
          .unwrap(),
      );
      return Ok(());
    }
  };

  let uuid = match Uuid::parse_str(noteid) {
    Ok(id) => id,
    Err(_e) => {
      usr.respond(
        http::Response::builder()
          .status(http::StatusCode::BAD_REQUEST)
          .header(
            http::header::CONTENT_TYPE,
            mime_type::MimeType::Txt.to_string().as_str(),
          )
          .body(format!("invalid note id {}: ", noteid).as_bytes().to_vec())
          .unwrap(),
      );
      return Ok(());
    }
  };

  let nid = match sqldata::note_id_for_uuid(&conn, &uuid) {
    Ok(c) => c,
    Err(e) => {
      return Err((usr, e));
    }
  };

  let hash = match sqldata::read_zknote_filehash(&conn, uid, nid) {
    Ok(Some(hash)) => hash,
    Ok(None) => {
      usr.respond(
        http::Response::builder()
          .status(http::StatusCode::NOT_FOUND)
          .body((format!("file {} not found!", nid)).as_bytes().to_vec())
          .unwrap(),
      );
      return Ok(());
    }
    Err(e) => {
      return Err((usr, e));
    }
  };

  let _zkln = match sqldata::read_zklistnote(&conn, &config_clone.file_path, uid, nid) {
    Ok(x) => x,
    Err(e) => {
      return Err((usr, e.into()));
    }
  };

  let stpath = config_clone.file_path.join(hash);

  match std::fs::read(stpath.as_path()) {
    Ok(v) => {
      // TODO: filename as in actix NAMED FILE.
      let r = match http::Response::builder().body(v) {
        Ok(r) => r,
        Err(e) => return Err((usr, zkerr::Error::String(format!("{}", e)))),
      };
      usr.respond(r);
      Ok(())
    }
    Err(e) => Err((usr, e.into())),
  }
}

#[tauri::command]
pub async fn timsg(
  app_handle: tauri::AppHandle,
  state: State<'_, ZkState>,
  msg: zt::TauriRequest,
) -> Result<zt::TauriReply, ()> {
  match timsg_err(app_handle, state, msg).await {
    Ok(ptd) => Ok(ptd),
    Err(e) => Ok(TauriReply::TyServerError(e.to_string())),
  }
}

pub async fn timsg_err(
  app_handle: tauri::AppHandle,
  state: State<'_, ZkState>,
  msg: zt::TauriRequest,
) -> Result<zt::TauriReply, zkerr::Error> {
  match msg {
    zt::TauriRequest::TrqUploadFiles => {
      // show open dialog
      if let Some(flz) = app_handle.dialog().file().blocking_pick_files() {
        let paths = flz
          .iter()
          .filter_map(|x| x.clone().into_path().ok())
          .collect();

        return make_file_notes(&state, &paths)
          .await
          .map(|fls| zt::TauriReply::TyUploadedFiles(fls));
      } else {
      }
    }
  }

  Ok(zt::TauriReply::TyUploadedFiles(zt::UploadedFiles {
    notes: Vec::new(),
  }))
}

async fn make_file_notes(
  state: &State<'_, ZkState>,
  // state: &zknotes_server_lib::state::State,
  files: &Vec<PathBuf>,
) -> Result<UploadedFiles, zkerr::Error> {
  let state = state.state.read().unwrap();
  let conn = sqldata::connection_open(state.config.orgauth_config.db.as_path())?;
  let uid = get_tauri_uid(&conn)?.ok_or(zkerr::Error::NotLoggedIn)?;

  let mut zklns = Vec::new();

  for pb in files {
    let name = pb.as_path().file_name().and_then(|x| x.to_str());

    if let Some(name) = name {
      let (nid64, _noteid, _fid) = sqldata::make_file_note(
        &conn,
        &state.server,
        &state.config.file_path,
        uid,
        &name.to_string(),
        pb,
        true,
      )?;

      // return zknoteedit.
      let listnote = sqldata::read_zklistnote(&conn, &state.config.file_path, Some(uid), nid64)?;

      zklns.push(listnote);
    }
  }
  Ok(UploadedFiles { notes: zklns })
}

#[tauri::command]
pub async fn zimsg(
  state: State<'_, ZkState>,
  msg: PrivateClosureRequest,
) -> Result<PrivateTimedData, ()> {
  let stateclone = state.state.clone();

  let res = std::thread::spawn(move || {
    let rt = actix_rt::System::new();
    let state = stateclone.write().unwrap();
    let zkres = tauri_zk_interface_loggedin(&state, &msg.request);
    match (
      rt.block_on(zkres),
      SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|n| n.as_millis()),
    ) {
      (Ok(sr), Ok(t)) => PrivateTimedData {
        utcmillis: t,
        data: PrivateClosureReply {
          closure_id: msg.closure_id,
          reply: sr,
        },
      },
      (Err(e), _) => PrivateTimedData {
        utcmillis: 0,
        data: PrivateClosureReply {
          closure_id: msg.closure_id,
          reply: PrivateReply::PvyServerError(PrivateError::PveString(e.to_string())),
        },
      },
      (_, Err(e)) => PrivateTimedData {
        utcmillis: 0,
        data: PrivateClosureReply {
          closure_id: msg.closure_id,
          reply: PrivateReply::PvyServerError(PrivateError::PveString(e.to_string())),
        },
      },
    }
  });

  Ok(res.join().unwrap())
}

pub async fn tauri_zk_interface_loggedin(
  state: &zknotes_server_lib::state::State,
  msg: &PrivateRequest,
) -> Result<PrivateReply, zkerr::Error> {
  let conn = sqldata::connection_open(state.config.orgauth_config.db.as_path())?;
  let uid =
    get_tauri_uid(&conn)?.ok_or(zkerr::Error::String("zimsg: not logged in".to_string()))?;

  zknotes_server_lib::interfaces::zk_interface_loggedin(&state, &conn, uid, &msg).await
}

#[tauri::command]
pub fn pimsg(state: State<ZkState>, msg: PublicRequest) -> PublicTimedData {
  println!("pimsg");

  match (
    zknotes_server_lib::interfaces::public_interface(
      &state.state.read().unwrap().config,
      &msg,
      None,
    ),
    SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|n| n.as_millis()),
  ) {
    (Ok(sr), Ok(t)) => PublicTimedData {
      utcmillis: t,
      data: sr,
    },
    (Err(e), Ok(t)) => PublicTimedData {
      utcmillis: t,
      data: PublicReply::PbyServerError(PublicError::PbeString(e.to_string())),
    },
    (_, Err(e)) => PublicTimedData {
      utcmillis: 0,
      data: PublicReply::PbyServerError(PublicError::PbeString(e.to_string())),
    },
  }
}

#[tauri::command]
pub fn uimsg(state: State<ZkState>, msg: UserRequest) -> UserResponse {
  println!("uimsg");

  match uimsg_err(state, msg) {
    Ok(ptd) => ptd,
    Err(e) => UserResponse::UrpServerError(e.to_string()),
  }
}

pub fn uimsg_err(state: State<ZkState>, msg: UserRequest) -> Result<UserResponse, zkerr::Error> {
  println!("uimsg");

  let conn = sqldata::connection_open(
    state
      .state
      .read()
      .unwrap()
      .config
      .orgauth_config
      .db
      .as_path(),
  )?;
  let ld = get_tauri_login_data(&conn, &mut sqldata::zknotes_callbacks())?;

  let mut ut = UuidTokener {
    uuid: ld.map(|ld| ld.uuid),
  };

  let ustate = state.state.read().unwrap().config.clone();

  // TODO pass &conn in instead of creating a second one in the fn.
  let sr = match tauri::async_runtime::block_on(zknotes_server_lib::interfaces::user_interface(
    &conn, &mut ut, &ustate, msg,
  )) {
    Ok(sr) => {
      match &sr {
        UserResponse::UrpLoggedIn(ld) => {
          set_single_value(&conn, "last_login", ld.userid.to_string().as_str())
            .map_err(|e| zkerr::annotate_string("error saving last login.".to_string(), e))?;
        }
        UserResponse::UrpLoggedOut => {
          set_single_value(&conn, "last_login", "").map_err(|e| {
            zkerr::annotate_string("error saving logged out status.".to_string(), e)
          })?;
        }
        _ => {}
      };
      sr
    }
    Err(e) => UserResponse::UrpServerError(e.to_string()),
  };

  Ok(sr)
}
