use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use light::api::LriSummary;
use light::session::LriSession;

#[derive(Clone)]
pub struct AppState {
	sessions: Arc<Mutex<HashMap<String, LriSession>>>,
}

impl AppState {
	pub fn new() -> Self {
		Self {
			sessions: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	pub fn open(&self, path: &str) -> Result<LriSummary, String> {
		let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;
		if !sessions.contains_key(path) {
			let session = LriSession::open(path).map_err(|e| e.to_string())?;
			sessions.insert(path.to_string(), session);
		}
		sessions
			.get(path)
			.ok_or_else(|| "session missing".to_string())?
			.summary()
			.map_err(|e| e.to_string())
	}

	pub fn with_session<T>(&self, path: &str, f: impl FnOnce(&LriSession) -> Result<T, String>) -> Result<T, String> {
		let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;
		if !sessions.contains_key(path) {
			let session = LriSession::open(path).map_err(|e| e.to_string())?;
			sessions.insert(path.to_string(), session);
		}
		let session = sessions
			.get(path)
			.ok_or_else(|| "session missing".to_string())?;
		f(session)
	}
}