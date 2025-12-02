use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, Duration, UNIX_EPOCH};
use std::hash::{Hash, Hasher};

/// Session data storage
/// 
/// Stores arbitrary key-value pairs for session data
pub type SessionData = HashMap<String, String>;

/// Session structure
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session ID
    pub id: String,
    
    /// Session data (key-value pairs)
    pub data: SessionData,
    
    /// Session creation time
    pub created_at: SystemTime,
    
    /// Session last access time
    pub last_access: SystemTime,
    
    /// Session expiration time
    pub expires_at: SystemTime,
}

impl Session {
    /// Create a new session with given ID
    fn new(id: String, timeout_secs: u64) -> Self {
        let now = SystemTime::now();
        Self {
            id,
            data: HashMap::new(),
            created_at: now,
            last_access: now,
            expires_at: now + Duration::from_secs(timeout_secs),
        }
    }

    /// Update last access time
    fn touch(&mut self, timeout_secs: u64) {
        let now = SystemTime::now();
        self.last_access = now;
        self.expires_at = now + Duration::from_secs(timeout_secs);
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }

    /// Get a value from session data
    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    /// Set a value in session data
    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    /// Remove a value from session data
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }

    /// Clear all session data
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

/// Session Manager
/// 
/// Manages HTTP sessions with in-memory storage.
/// Thread-safe implementation using Arc<RwLock> for concurrent access.
pub struct SessionManager {
    /// Session storage: session_id -> Session
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    
    /// Session timeout in seconds
    timeout_secs: u64,
    
    /// Cookie name for session ID (default: "session_id")
    cookie_name: String,
}

impl SessionManager {
    /// Create a new SessionManager with default timeout
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            timeout_secs,
            cookie_name: "session_id".to_string(),
        }
    }

    /// Create a new SessionManager with custom cookie name
    pub fn with_cookie_name(timeout_secs: u64, cookie_name: String) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            timeout_secs,
            cookie_name,
        }
    }

    /// Generate a unique session ID
    fn generate_session_id() -> String {
        use std::collections::hash_map::DefaultHasher;
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Generate random component
        let mut hasher = DefaultHasher::new();
        timestamp.hash(&mut hasher);
        let random: u64 = std::ptr::addr_of!(hasher) as usize as u64;
        random.hash(&mut hasher);
        
        let hash = hasher.finish();
        format!("{:x}_{:x}", timestamp, hash)
    }

    /// Create a new session and return its ID
    pub fn create_session(&self) -> String {
        let session_id = Self::generate_session_id();
        let session = Session::new(session_id.clone(), self.timeout_secs);
        
        let mut sessions = self.sessions.write().unwrap();
        sessions.insert(session_id.clone(), session);
        
        session_id
    }

    /// Get or create a session from request cookie
    /// 
    /// If session ID exists in cookies and session is valid, returns existing session.
    /// Otherwise creates a new session.
    pub fn get_or_create_session(&self, session_id: Option<&str>) -> Option<String> {
        if let Some(id) = session_id {
            // Try to get existing session
            let sessions = self.sessions.read().unwrap();
            if let Some(session) = sessions.get(id) {
                if !session.is_expired() {
                    // Session exists and is valid
                    drop(sessions);
                    // Touch the session to update last access
                    self.touch_session(id);
                    return Some(id.to_string());
                }
            }
        }
        
        // Create new session
        Some(self.create_session())
    }

    /// Get session by ID (returns a clone of session data)
    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        let mut sessions = self.sessions.write().unwrap();
        
        if let Some(session) = sessions.get_mut(session_id) {
            if session.is_expired() {
                // Remove expired session
                sessions.remove(session_id);
                return None;
            }
            
            // Touch session and return clone
            session.touch(self.timeout_secs);
            return Some(session.clone());
        }
        
        None
    }

    /// Update session data
    pub fn update_session(&self, session_id: &str, key: String, value: String) -> Result<(), String> {
        let mut sessions = self.sessions.write().unwrap();
        
        if let Some(session) = sessions.get_mut(session_id) {
            if session.is_expired() {
                sessions.remove(session_id);
                return Err("Session expired".to_string());
            }
            
            session.touch(self.timeout_secs);
            session.set(key, value);
            Ok(())
        } else {
            Err("Session not found".to_string())
        }
    }

    /// Remove a value from session
    pub fn remove_from_session(&self, session_id: &str, key: &str) -> Result<Option<String>, String> {
        let mut sessions = self.sessions.write().unwrap();
        
        if let Some(session) = sessions.get_mut(session_id) {
            if session.is_expired() {
                sessions.remove(session_id);
                return Err("Session expired".to_string());
            }
            
            session.touch(self.timeout_secs);
            Ok(session.remove(key))
        } else {
            Err("Session not found".to_string())
        }
    }

    /// Delete a session
    pub fn delete_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().unwrap();
        sessions.remove(session_id);
    }

    /// Touch (update last access time) a session
    fn touch_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            if !session.is_expired() {
                session.touch(self.timeout_secs);
            } else {
                sessions.remove(session_id);
            }
        }
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) -> usize {
        let mut sessions = self.sessions.write().unwrap();
        let initial_count = sessions.len();
        
        sessions.retain(|_, session| !session.is_expired());
        
        initial_count - sessions.len()
    }

    /// Get cookie name for session ID
    pub fn cookie_name(&self) -> &str {
        &self.cookie_name
    }

    /// Get session timeout in seconds
    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let manager = SessionManager::new(3600);
        let session_id = manager.create_session();
        
        assert!(!session_id.is_empty());
        
        let session = manager.get_session(&session_id);
        assert!(session.is_some());
        assert!(!session.unwrap().is_expired());
    }

    #[test]
    fn test_session_data() {
        let manager = SessionManager::new(3600);
        let session_id = manager.create_session();
        
        manager.update_session(&session_id, "user".to_string(), "john".to_string()).unwrap();
        
        let session = manager.get_session(&session_id).unwrap();
        assert_eq!(session.get("user"), Some(&"john".to_string()));
    }

    #[test]
    fn test_session_expiration() {
        let manager = SessionManager::new(1); // 1 second timeout
        let session_id = manager.create_session();
        
        // Session should exist
        assert!(manager.get_session(&session_id).is_some());
        
        // Wait for expiration (simplified test - in real scenario use mock time)
        std::thread::sleep(Duration::from_secs(2));
        
        // Cleanup expired sessions
        manager.cleanup_expired();
        
        // Session should be removed
        assert!(manager.get_session(&session_id).is_none());
    }

    #[test]
    fn test_get_or_create_session() {
        let manager = SessionManager::new(3600);
        
        // Create new session
        let session_id1 = manager.get_or_create_session(None).unwrap();
        assert!(!session_id1.is_empty());
        
        // Get existing session
        let session_id2 = manager.get_or_create_session(Some(&session_id1)).unwrap();
        assert_eq!(session_id1, session_id2);
    }
}
