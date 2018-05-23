use std::fmt::Debug;
use std::default::Default;

use actix_web::http::header::IntoHeaderValue;

pub trait Challenge: 'static + Debug + Clone + Send + Sync + IntoHeaderValue + Default {}