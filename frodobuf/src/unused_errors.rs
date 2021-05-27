// Copyright 2015-2020 Capital One Services, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Errors
//!
//! This module contains types and utility functions for error handling

use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub struct Error(Box<ErrorKind>);

pub fn new(kind: ErrorKind) -> Error {
    Error(Box::new(kind))
}

#[derive(Debug)]
pub enum ErrorKind {
    UTF8(std::string::FromUtf8Error),
    UTF8Str(std::str::Utf8Error),
    HostError(String),
    BadDispatch(String),
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        &self.0
    }

    pub fn into_kind(self) -> ErrorKind {
        *self.0
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self.0 {
            ErrorKind::UTF8(_) => "UTF8 encoding failure",
            ErrorKind::UTF8Str(_) => "UTF8 encoding failure",
            ErrorKind::HostError(_) => "Host Error",
            ErrorKind::BadDispatch(_) => "Bad dispatch",
        }
    }

    fn cause(&self) -> Option<&dyn StdError> {
        match *self.0 {
            ErrorKind::UTF8(ref e) => Some(e),
            ErrorKind::UTF8Str(ref e) => Some(e),
            ErrorKind::HostError(_) => None,
            ErrorKind::BadDispatch(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            ErrorKind::UTF8(ref e) => write!(f, "UTF8 encoding error: {}", e),
            ErrorKind::UTF8Str(ref e) => write!(f, "UTF8 error: {}", e),
            ErrorKind::HostError(ref e) => write!(f, "Host error: {}", e),
            ErrorKind::BadDispatch(ref e) => write!(f, "Bad dispatch, attempted operation: {}", e),
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(source: std::str::Utf8Error) -> Error {
        Error(Box::new(ErrorKind::UTF8Str(source)))
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(source: std::string::FromUtf8Error) -> Error {
        Error(Box::new(ErrorKind::UTF8(source)))
    }
}
