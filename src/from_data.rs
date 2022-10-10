// use rocket::data::{FromData, Outcome};
// use rocket::{Data, data, Request, request, State};
// use rocket::http::{ContentType, Status};
// use rocket::serde::json::Json;
// use rocket::serde::Serialize;
// use rocket::serde::Deserialize;
// use serde_json::Value;
// use utoipa::Component;
// use crate::{Context, ControllerClient, EvmClient, ExecutorClient};
// use crate::api::CreateContract;
// use crate::core::account::MaybeLocked;
// use crate::core::controller::TransactionSenderBehaviour;
// use crate::error::CacheError;
// use crate::from_request::CacheResult;
//
// #[derive(Debug)]
// pub enum Error {
//     TooLarge,
//     NoColon,
//     InvalidAge,
//     Io(std::io::Error),
// }
//
//
//
// #[rocket::async_trait]
// impl<'r> FromData<'r> for MaybeLocked {
//     type Error = Error;
//
//     async fn from_data(req: &'r Request<'_>, data: Data<'r>) -> Outcome<'r, Self> {
//         let address = req.headers().get("account").next();
//         let ctx = req
//             .guard::<&State<Context<ControllerClient, ExecutorClient, EvmClient>>>()
//             .await
//             .unwrap();
//
//         use r2d2_redis::redis::Commands;
//         let content: String = ctx.get_redis_connection().hget("accounts", address.unwrap()).unwrap();
//         use Error::*;
//         use rocket::outcome::Outcome::*;
//         Success(toml::from_str(content.as_str()).unwrap())
//
//         //
//         // // Ensure the content type is correct before opening the data.
//         // if req.content_type() != Some(&ContentType::JSON) {
//         //     return Forward(data);
//         // }
//         //
//         // // Use a configured limit with name 'person' or fallback to default.
//         // let limit = req.limits().get("json").unwrap();
//         //
//         // // Read the data into a string.
//         // let json_str = match data.open(limit).into_string().await {
//         //     Ok(string) if string.is_complete() => string.into_inner(),
//         //     Ok(_) => return Failure((Status::PayloadTooLarge, TooLarge)),
//         //     Err(e) => return Failure((Status::InternalServerError, Io(e))),
//         // };
//         //
//         // // We store `string` in request-local cache for long-lived borrows.
//         // let json_str = request::local_cache!(req, json_str);
//         //
//         // // Split the string into two pieces at ':'.
//         // match serde_json::from_str(json_str) {
//         //     Ok(data) => {
//         //         println!("{}", json_str);
//         //         Success(CacheResult::Ok(data))
//         //     }
//         //     Err(e) => Success(CacheResult::Err(CacheError::Deserialize(e))),
//         // }
//
//     }
// }
