use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;
use tauri::{http, utils::mime_type};
use tauri::{State, UriSchemeResponder};
use uuid::Uuid;
use zknotes_server_lib::error as zkerr;
use zknotes_server_lib::orgauth::data::{LoginData, UserRequestMessage};
use zknotes_server_lib::orgauth::endpoints::UuidTokener;
use zknotes_server_lib::sqldata::{get_single_value, set_single_value};
use zknotes_server_lib::zkprotocol::messages::{
  PrivateMessage, PrivateReplies, PrivateReplyMessage, PublicMessage, PublicReplies,
  PublicReplyMessage,
};
use zknotes_server_lib::{sqldata, UserResponse, UserResponseMessage};

pub struct ZkState {
  pub state: Arc<RwLock<zknotes_server_lib::state::State>>,
  pub uid: Mutex<Option<i64>>,
}

#[tauri::command]
pub fn greet(name: &str) -> String {
  println!("greeet");
  format!("Hello, {}!", name)
}

#[tauri::command]
pub fn login_data(state: State<ZkState>) -> Result<Option<LoginData>, String> {
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
      return Err(e.to_string());
    }
  };

  get_single_value(&conn, "last_login")
    .and_then(|x| Ok(x.and_then(|s| serde_json::from_str::<LoginData>(s.as_str()).ok())))
    .map_err(|e| e.to_string())
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

  let uid = Some(2);

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
pub fn zimsg(state: State<ZkState>, msg: PrivateMessage) -> PrivateTimedData {
  // gonna need config obj, uid.
  // uid could be passed from elm maybe.

  println!("zimsg");

  let stateclone = state.state.clone();

  // let res = tauri::async_runtime::block_on(async move {
  let res = std::thread::spawn(move || {
    let rt = actix_rt::System::new();
    let state = stateclone.write().unwrap();
    // let serv = atomic_server_lib::serve::serve(config_clone);
    let zkres = zknotes_server_lib::interfaces::zk_interface_loggedin(&state, 2, &msg);
    match (
      rt.block_on(zkres),
      SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|n| n.as_millis()),
    ) {
      (Ok(sr), Ok(t)) => {
        println!("sr: {:?}", sr.what);
        // serde_json::to_value(&sr).unwrap());
        PrivateTimedData {
          utcmillis: t,
          data: sr,
        }
      }
      (Err(e), _) => PrivateTimedData {
        utcmillis: 0,
        data: PrivateReplyMessage {
          what: PrivateReplies::ServerError,
          content: Value::String(e.to_string()),
        },
      },
      (_, Err(e)) => PrivateTimedData {
        utcmillis: 0,
        data: PrivateReplyMessage {
          what: PrivateReplies::ServerError,
          content: Value::String(e.to_string()),
        },
      },
    }
  });

  res.join().unwrap()
}

#[tauri::command]
pub fn pimsg(state: State<ZkState>, msg: PublicMessage) -> PublicTimedData {
  // gonna need config obj, uid.
  // uid could be passed from elm maybe.

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
      println!("sr: {:?}", sr.what);
      // serde_json::to_value(&sr).unwrap());
      PublicTimedData {
        utcmillis: t,
        data: sr,
      }
    }
    (Err(e), _) => PublicTimedData {
      utcmillis: 0,
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
  // gonna need config obj, uid.
  // uid could be passed from elm maybe.

  println!("uimsg");

  let mut ut = UuidTokener { uuid: None };

  let ustate = state.state.read().unwrap().config.clone();

  let sr = match tauri::async_runtime::block_on(zknotes_server_lib::interfaces::user_interface(
    &mut ut, &ustate, msg,
  )) {
    Ok(sr) => {
      // serde_json::to_value(&sr).unwrap());
      match (&sr.what, sr.data.clone()) {
        (UserResponse::LoggedIn, Some(d)) => {
          let conn = match sqldata::connection_open(ustate.orgauth_config.db.as_path()) {
            Ok(c) => c,
            Err(e) => {
              return UserResponseMessage {
                what: UserResponse::ServerError,
                data: Some("unable to connect to database.".into()),
              };
            }
          };
          match set_single_value(&conn, "last_login", d.to_string().as_str()) {
            Ok(c) => c,
            Err(_e) => {
              return UserResponseMessage {
                what: UserResponse::ServerError,
                data: Some("error saving last login.".into()),
              };
            }
          };
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

  sr
}

// #[tauri::command]
// pub fn aimsg(msg: UserMessage) -> ServerResponse {
//   // gonna need config obj, uid.
//   // uid could be passed from elm maybe.

//   println!("aimsg");

//   let c = zknotes_server_lib::defcon();

//   match zknotes_server_lib::interfaces::admin_interface(&c, 2, &msg) {
//     Ok(sr) => {
//       println!("sr: {}", sr.what);
//       // serde_json::to_value(&sr).unwrap());
//       sr
//     }
//     Err(e) => ServerResponse {
//       what: "erro".to_string(),
//       content: Value::String("erro".to_string()),
//     },
//   }
// }
