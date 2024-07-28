use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::Read;
// use actix_files::NamedFile;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;
use tauri::{State, UriSchemeResponder};
use zknotes_server_lib::orgauth::data::UserRequestMessage;
use zknotes_server_lib::{sqldata, UserResponse, UserResponseMessage};
// use zknotes_server_lib::err_main;
// use zknotes_server_lib::orgauth::data::WhatMessage;
use tauri::{http, utils::mime_type, Manager};
use uuid::Uuid;
use zknotes_server_lib::error as zkerr;
use zknotes_server_lib::orgauth::endpoints::{Callbacks, Tokener, UuidTokener};
use zknotes_server_lib::zkprotocol::messages::{
  PrivateMessage, PrivateReplies, PrivateReplyMessage, PublicMessage, PublicReplies,
  PublicReplyMessage,
};

pub struct ZkState {
  pub config: Mutex<zknotes_server_lib::config::Config>,
  pub uid: Mutex<Option<i64>>,
}

#[tauri::command]
pub fn greet(name: &str) -> String {
  println!("greeet");
  format!("Hello, {}!", name)
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
// pub fn fileresp(state: State<ZkState>, uid: i64, noteid: &str) -> http::Response {
// pub fn fileresp(
//   state: State<ZkState>,
//   request: tauri::http::Request<Vec<u8>>,
//   usr: UriSchemeResponder,
// ) {
//   match fileresp_helper(state, request, usr) {
//     Ok(()) => (),
//     Err(e) => {
//       usr.respond(
//         http::Response::builder()
//           .status(http::StatusCode::INTERNAL_SERVER_ERROR)
//           .body(format!("{:?}", e).as_bytes().to_vec())
//           .unwrap(),
//       );
//     }
//   }
// }
pub fn fileresp(
  state: State<ZkState>,
  request: tauri::http::Request<Vec<u8>>,
  usr: UriSchemeResponder,
) {
  println!("fr1");
  // async fn file(session: Session, config: web::Data<Config>, req: HttpRequest) -> HttpResponse {
  let conn =
    match sqldata::connection_open(state.config.lock().unwrap().orgauth_config.db.as_path()) {
      Ok(c) => c,
      Err(e) => {
        usr.respond(
          http::Response::builder()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(format!("{:?}", e).as_bytes().to_vec())
            .unwrap(),
        );
        return;
      }
    };
  println!("fr2");

  // let errsponse = |e| {
  //   usr.respond(
  //     http::Response::builder()
  //       .status(http::StatusCode::INTERNAL_SERVER_ERROR)
  //       .body(format!("{:?}", e).as_bytes().to_vec())
  //       .unwrap(),
  //   );
  // };
  // let suser = match session_user(&conn, session, &config)? {
  //   Either::Left(user) => Some(user),
  //   Either::Right(_sr) => None,
  // };
  // let uid = suser.map(|user| user.id);

  let config_clone = state.config.lock().unwrap().clone();
  println!("fr3");

  let uid = Some(2);

  // let v12 = "/d5d6bba3-cba5-4eb1-9caf-be693c2277b5".to_string();
  // let v: Vec<&str> = v12.split("/").collect();
  // println!("split: {:?} ", v);

  // println!(
  //   "pre noteid: {:?}  {:?}",
  //   request.uri().path_and_query().map(|pnq| pnq.path()),
  //   request
  //     .uri()
  //     .path_and_query()
  //     .map(|pnq| pnq.path())
  //     .map(|p| p.split("/"))
  //     .map(|mut s| s.collect(s)) // .map(|mut s| (s.nth(0), s.nth(1), s.nth(2), s.nth(3)))
  // );

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
          .status(http::StatusCode::INTERNAL_SERVER_ERROR)
          .body("file id required: /file/<id>".as_bytes().to_vec())
          .unwrap(),
      );
      return;
    }
  };

  println!("noteid: {}", noteid);

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
      return;
    } // HttpResponse::BadRequest().body(e.to_string())
  };
  let nid = match sqldata::note_id_for_uuid(&conn, &uuid) {
    Ok(c) => c,
    Err(e) => {
      return usr.respond(
        http::Response::builder()
          .status(http::StatusCode::INTERNAL_SERVER_ERROR)
          .body(format!("{:?}", e).as_bytes().to_vec())
          .unwrap(),
      )
    }
  };
  let hash = match sqldata::read_zknote_filehash(&conn, uid, nid) {
    Ok(Some(hash)) => hash,
    Ok(None) => {
      usr.respond(
        http::Response::builder()
          .status(http::StatusCode::INTERNAL_SERVER_ERROR)
          .body((format!("file {} not found!", nid)).as_bytes().to_vec())
          .unwrap(),
      );
      return;
    }
    Err(e) => {
      return usr.respond(
        http::Response::builder()
          .status(http::StatusCode::INTERNAL_SERVER_ERROR)
          .body(format!("{:?}", e).as_bytes().to_vec())
          .unwrap(),
      )
    }
  };

  let zkln = match sqldata::read_zklistnote(&conn, &config_clone.file_path, uid, nid) {
    Ok(x) => x,
    Err(e) => {
      return usr.respond(
        http::Response::builder()
          .status(http::StatusCode::INTERNAL_SERVER_ERROR)
          .body(format!("{:?}", e).as_bytes().to_vec())
          .unwrap(),
      )
    }
  };

  let stpath = config_clone.file_path.join(hash);

  println!("reading path: {:?} ", stpath);

  match std::fs::read(stpath.as_path()) {
    Ok(v) => {
      usr.respond(http::Response::builder().body(v).unwrap());
    }
    Err(e) => {
      return usr.respond(
        http::Response::builder()
          .status(http::StatusCode::INTERNAL_SERVER_ERROR)
          .body(format!("{:?}", e).as_bytes().to_vec())
          .unwrap(),
      )
    }
  }

  // let mut f = match File::open(stpath) {
  //   Ok(f) => f,
  //   Err(e) => {
  //     return usr.respond(
  //       http::Response::builder()
  //         .status(http::StatusCode::INTERNAL_SERVER_ERROR)
  //         .body(format!("{:?}", e).as_bytes().to_vec())
  //         .unwrap(),
  //     )
  //   }
  // };
  // // .and_then(|f| NamedFile::from_file(f, Path::new(zkln.title.as_str())))?;
  // let mut v = Vec::new();
  // match f.read(&mut v) {
  //   Ok(s) => {
  //     println!("read size: {}", s);
  //   }
  //   Err(e) => {
  //     return usr.respond(
  //       http::Response::builder()
  //         .status(http::StatusCode::INTERNAL_SERVER_ERROR)
  //         .body(format!("{:?}", e).as_bytes().to_vec())
  //         .unwrap(),
  //     )
  //   }
  // };

  // println!("returning file");
  // usr.respond(http::Response::builder().body(v).unwrap());
  // Ok(())
  // Err(e) => HttpResponse::NotFound().body(format!("{:?}", e)),
}

#[tauri::command]
pub fn zimsg(state: State<ZkState>, msg: PrivateMessage) -> PrivateTimedData {
  // gonna need config obj, uid.
  // uid could be passed from elm maybe.

  println!("zimsg");

  let config_clone = state.config.lock().unwrap().clone();

  // let res = tauri::async_runtime::block_on(async move {
  let res = std::thread::spawn(move || {
    let rt = actix_rt::System::new();
    // let serv = atomic_server_lib::serve::serve(config_clone);
    let zkres = zknotes_server_lib::interfaces::zk_interface_loggedin(&config_clone, 2, &msg);
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

  // match (
  //   // tauri::async_runtime::block_on(zknotes_server_lib::interfaces::zk_interface_loggedin(
  //   //   &&state.config.lock().unwrap(),
  //   //   2,
  //   //   &msg,
  //   // )),
  //   res,
  //   SystemTime::now()
  //     .duration_since(SystemTime::UNIX_EPOCH)
  //     .map(|n| n.as_millis()),
  // ) {
  //   (Ok(sr), Ok(t)) => {
  //     println!("sr: {:?}", sr.what);
  //     // serde_json::to_value(&sr).unwrap());
  //     PrivateTimedData {
  //       utcmillis: t,
  //       data: sr,
  //     }
  //   }
  //   (Err(e), _) => PrivateTimedData {
  //     utcmillis: 0,
  //     data: PrivateReplyMessage {
  //       what: PrivateReplies::ServerError,
  //       content: Value::String(e.to_string()),
  //     },
  //   },
  //   (_, Err(e)) => PrivateTimedData {
  //     utcmillis: 0,
  //     data: PrivateReplyMessage {
  //       what: PrivateReplies::ServerError,
  //       content: Value::String(e.to_string()),
  //     },
  //   },
  // }
}

#[tauri::command]
pub fn pimsg(state: State<ZkState>, msg: PublicMessage) -> PublicTimedData {
  // gonna need config obj, uid.
  // uid could be passed from elm maybe.

  println!("pimsg");

  match (
    zknotes_server_lib::interfaces::public_interface(&state.config.lock().unwrap(), msg, None),
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

  let sr = match tauri::async_runtime::block_on(zknotes_server_lib::interfaces::user_interface(
    &mut ut,
    &state.config.lock().unwrap(),
    msg,
  )) {
    Ok(sr) => {
      println!("sr: {:?}", sr.what);
      // serde_json::to_value(&sr).unwrap());
      sr
    }
    Err(e) => UserResponseMessage {
      what: UserResponse::ServerError,
      data: Some(Value::String(e.to_string())),
    },
  };

  println!("ut {:?}", ut.uuid);

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
