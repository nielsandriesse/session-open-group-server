use std::convert::TryFrom;

use serde::Deserialize;
use warp::{Filter, http::StatusCode, Rejection};

use super::crypto;
use super::handlers;
use super::lsrpc;
use super::models;
use super::storage;

#[derive(Debug, Deserialize)]
pub struct QueryOptions {
    pub limit: Option<u16>,
    pub from_server_id: Option<i64>
}

#[derive(Debug)]
pub struct InvalidRequestError;
impl warp::reject::Reject for InvalidRequestError { }

pub async fn handle_rpc_call(rpc_call: lsrpc::RpcCall, pool: &storage::DatabaseConnectionPool) -> Result<impl warp::Reply, Rejection> {
    // Check that the endpoint is a valid URI
    let uri = match rpc_call.endpoint.parse::<http::Uri>() {
        Ok(uri) => uri,
        Err(e) => {
            println!("Couldn't parse URI from: {:?} due to error: {:?}.", rpc_call.endpoint, e);
            return Err(warp::reject::custom(InvalidRequestError));
        }
    };
    // Switch on the HTTP method
    match rpc_call.method.as_ref() {
        "GET" => return handle_get_rpc_call(rpc_call, uri, pool).await,
        "POST" => return handle_post_rpc_call(rpc_call, uri, pool).await,
        "DELETE" => return handle_delete_rpc_call(rpc_call, uri, pool).await,
        _ => {
            println!("Ignoring RPC call with invalid or unused HTTP method: {:?}.", rpc_call.method);
            return Err(warp::reject::custom(InvalidRequestError));
        }
    }
}

pub async fn handle_get_rpc_call(rpc_call: lsrpc::RpcCall, uri: http::Uri, pool: &storage::DatabaseConnectionPool) -> Result<warp::reply::Json, Rejection> {
    // Parse query options if needed
    let mut query_options = QueryOptions { limit : None, from_server_id : None };
    if let Some(query) = uri.query() {
        query_options = match serde_json::from_str(&query) {
            Ok(query_options) => query_options,
            Err(e) => {
                println!("Couldn't parse query options from: {:?} due to error: {:?}.", query, e);
                return Err(warp::reject::custom(InvalidRequestError));
            }
        };
    }
    // Switch on the path
    match uri.path() {
        "/messages" => return handlers::get_messages(query_options, pool).await,
        "/deleted_messages" => return handlers::get_deleted_messages(query_options, pool).await,
        "/moderators" => return handlers::get_moderators(pool).await,
        "/block_list" => return handlers::get_banned_public_keys(pool).await,
        "/member_count" => return handlers::get_member_count(pool).await,
        _ => {
            println!("Ignoring RPC call with invalid or unused endpoint: {:?}.", rpc_call.endpoint);
            return Err(warp::reject::custom(InvalidRequestError));        
        }
    }
}

pub async fn handle_post_rpc_call(rpc_call: lsrpc::RpcCall, uri: http::Uri, pool: &storage::DatabaseConnectionPool) -> Result<impl warp::Reply, Rejection> {
    match uri.path() {
        "/messages" => {
            let message = match serde_json::from_str(&rpc_call.body) {
                Ok(query_options) => query_options,
                Err(e) => {
                    println!("Couldn't parse message from: {:?} due to error: {:?}.", rpc_call.body, e);
                    return Err(warp::reject::custom(InvalidRequestError));
                }
            };
            return handlers::insert_message(message, pool).await; 
        },
        "/block_list" => return handlers::ban(rpc_call.body, pool).await,
        _ => {
            println!("Ignoring RPC call with invalid or unused endpoint: {:?}.", rpc_call.endpoint);
            return Err(warp::reject::custom(InvalidRequestError));        
        }
    }
}

pub async fn handle_delete_rpc_call(rpc_call: lsrpc::RpcCall, uri: http::Uri, pool: &storage::DatabaseConnectionPool) -> Result<StatusCode, Rejection> {
    // DELETE /messages/:server_id
    if uri.path().starts_with("/messages") {
        let components: Vec<&str> = uri.path()[1..].split("/").collect(); // Drop the leading slash and split on subsequent slashes
        if components.len() != 2 {
            println!("Invalid endpoint: {:?}.", rpc_call.endpoint);
            return Err(warp::reject::custom(InvalidRequestError));
        }
        let server_id: i64 = match components[1].parse() {
            Ok(server_id) => server_id,
            Err(e) => {
                println!("Invalid endpoint: {:?}.", rpc_call.endpoint);
                return Err(warp::reject::custom(InvalidRequestError));
            }
        };
        return handlers::delete_message(server_id, pool).await;
    }
    // DELETE /block_list/:public_key
    if uri.path().starts_with("/block_list") {
        let components: Vec<&str> = uri.path()[1..].split("/").collect(); // Drop the leading slash and split on subsequent slashes
        if components.len() != 2 {
            println!("Invalid endpoint: {:?}.", rpc_call.endpoint);
            return Err(warp::reject::custom(InvalidRequestError));
        }
        let public_key = components[1].to_string();
        return handlers::unban(public_key, pool).await;
    }
    // Unrecognized endpoint
    println!("Invalid endpoint: {:?}.", rpc_call.endpoint);
    return Err(warp::reject::custom(InvalidRequestError));
}