use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tauri::{http, utils::mime_type};
use tauri::{State, UriSchemeResponder};
use uuid::Uuid;
use zknotes_server_lib::error as zkerr;
use zknotes_server_lib::orgauth::data::{LoginData, UserRequestMessage};
use zknotes_server_lib::orgauth::dbfun;
use zknotes_server_lib::orgauth::endpoints::{Callbacks, UuidTokener};
use zknotes_server_lib::rusqlite::{bypass_sqlite_initialization, Connection};
use zknotes_server_lib::sqldata::{get_single_value, set_single_value};
use zknotes_server_lib::zkprotocol::messages::{
  PrivateMessage, PrivateReplies, PrivateReplyMessage, PublicMessage, PublicReplies,
  PublicReplyMessage,
};
use zknotes_server_lib::{sqldata, UserResponse, UserResponseMessage};

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

pub fn get_tauri_uid(conn: &Connection) -> Result<Option<i64>, zkerr::Error> {
  get_single_value(&conn, "last_login")
    .and_then(|x| Ok(x.and_then(|s| serde_json::from_str::<i64>(s.as_str()).ok())))
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
  ld.data = data;
  Ok(Some(ld))
}

#[derive(Serialize, Deserialize)]
pub struct PrivateTimedData {
  utcmillis: u128,
  data: PrivateReplyMessage,
}
#[derive(Serialize, Deserialize)]
pub struct PublicTimedData {
  utcmillis: u128,
  data: PublicReplyMessage,
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
pub fn zimsg(state: State<'_, ZkState>, msg: PrivateMessage) -> PrivateTimedData {
  println!("zimsg");

  match zimsg_err(state, msg) {
    Ok(ptd) => ptd,
    Err(e) => PrivateTimedData {
      utcmillis: 0,
      data: PrivateReplyMessage {
        what: PrivateReplies::ServerError,
        content: Value::String(e.to_string()),
      },
    },
  }
}

pub fn zimsg_err(
  state: State<'_, ZkState>,
  msg: PrivateMessage,
) -> Result<PrivateTimedData, zkerr::Error> {
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
  let uid =
    get_tauri_uid(&conn)?.ok_or(zkerr::Error::String("zimsg: not logged in".to_string()))?;

  let sr = tauri::async_runtime::block_on(zknotes_server_lib::interfaces::zk_interface_loggedin(
    &state.state.read().unwrap(), // TODO fix
    uid,
    &msg,
  ));

  let dt = sr?;

  let st = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)?
    .as_millis();

  Ok(PrivateTimedData {
    utcmillis: st,
    data: dt,
  })
}

#[tauri::command]
pub fn pimsg(state: State<ZkState>, msg: PublicMessage) -> PublicTimedData {
  println!("pimsg");

  match (
    zknotes_server_lib::interfaces::public_interface(
      &state.state.read().unwrap().config,
      msg,
      None,
    ),
    SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|n| n.as_millis()),
  ) {
    (Ok(sr), Ok(t)) => {
      // serde_json::to_value(&sr).unwrap());
      PublicTimedData {
        utcmillis: t,
        data: sr,
      }
    }
    (Err(e), Ok(t)) => PublicTimedData {
      utcmillis: t,
      data: PublicReplyMessage {
        what: PublicReplies::ServerError,
        content: Value::String(e.to_string()),
      },
    },
    (_, Err(e)) => PublicTimedData {
      utcmillis: 0,
      data: PublicReplyMessage {
        what: PublicReplies::ServerError,
        content: Value::String(e.to_string()),
      },
    },
  }
}

#[tauri::command]
pub fn uimsg(state: State<ZkState>, msg: UserRequestMessage) -> UserResponseMessage {
  println!("uimsg");

  match uimsg_err(state, msg) {
    Ok(ptd) => ptd,
    Err(e) => UserResponseMessage {
      what: UserResponse::ServerError,
      data: Some(Value::String(e.to_string())),
    },
  }
}
pub fn uimsg_err(
  state: State<ZkState>,
  msg: UserRequestMessage,
) -> Result<UserResponseMessage, zkerr::Error> {
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

  let sr = match tauri::async_runtime::block_on(zknotes_server_lib::interfaces::user_interface(
    &mut ut, &ustate, msg,
  )) {
    Ok(sr) => {
      match (&sr.what, sr.data.clone()) {
        (UserResponse::LoggedIn, Some(d)) => {
          let ld = serde_json::from_value::<LoginData>(d)?;
          set_single_value(&conn, "last_login", ld.userid.to_string().as_str())
            .map_err(|e| zkerr::annotate_string("error saving last login.".to_string(), e))?;
        }
        (UserResponse::LoggedOut, _) => {
          set_single_value(&conn, "last_login", "").map_err(|e| {
            zkerr::annotate_string("error saving logged out status.".to_string(), e)
          })?;
        }
        _ => {}
      }
      sr
    }
    Err(e) => UserResponseMessage {
      what: UserResponse::ServerError,
      data: Some(Value::String(e.to_string())),
    },
  };

  Ok(sr)
}
