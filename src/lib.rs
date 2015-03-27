#![feature(core, std_misc)]

extern crate "rustc-serialize" as rustc_serialize;
extern crate postgres;
extern crate threadpool;

use std::env;
use rustc_serialize::json::decode;
use std::fs::File;
use std::io::Read;
// use postgres::*;
use postgres::{SslMode,ToSql,Rows};
use std::sync::Mutex;
use std::path::Path;
use std::result::Result;
use threadpool::ScopedPool;
use std::sync::Semaphore;
use std::error::Error;
use std::thread;
use std::sync::Arc;

#[test]
fn it_works() {}
pub struct PoolError{
	pub message: String
}
impl PoolError{
	fn new(msg: String) -> PoolError{
		PoolError{
			message: msg
		}
	}
}

pub struct Pool{
	conn_string: String,
	conn_semaphore: Semaphore,
	conn_pool: Mutex<Vec<postgres::Connection>>,
	num_connections: u32,
	max_connections: u32
}
impl Pool{
	pub fn new(max: u32, conn_string: &str) -> Pool{
		Pool{
			conn_string: conn_string.to_string(),
			conn_semaphore: Semaphore::new(max as isize),
			conn_pool: Mutex::new(Vec::new()),
			num_connections: 0,
			max_connections: max
		}
	}

	fn new_connection(conn_string: &str) -> Result<postgres::Connection, PoolError>{
		match postgres::Connection::connect(conn_string, &SslMode::None){
			Ok(conn) => Ok(conn),
			Err(_) => Err(PoolError::new("failed connecting new connection to postgres database".to_string()))
		}
	}

	pub fn query<Func,Ret>(&self, query_string: &str, params: &[&ToSql], mut func: Func) -> Result<Ret,PoolError>
	where Func: FnMut(Rows) -> Ret{
		let connection;
		{//inner to to unlock mutex after connection is aquired

			//make sure we haven't reached max connections
			let _sem_guard = self.conn_semaphore.access();
			connection = match self.conn_pool.lock().unwrap().pop(){
				Some(conn) => conn,
				None => {
					try!(Pool::new_connection(&self.conn_string))
				}
			}
		}
		let stmt = match connection.prepare(query_string){
			Ok(stmt) => stmt,
			Err(err) => return Err(PoolError::new(format!("error preparing query: {}", err.description())))
		};
		let result = match stmt.query(params){
			Ok(result) => result,
			Err(err) => return Err(PoolError::new(
				format!("error running query: {} for query string {}", err.description(), query_string)
			))
		};
		Ok(func(result))
	}
}