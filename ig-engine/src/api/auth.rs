#![allow(dead_code)]
use chrono::{DateTime, Utc};
use std::time::Duration;
use tracing::{debug, info};

use crate::api::rest_client::IGRestClient;

/// Session environment enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Demo,
    Production,
}

impl Environment {
    pub fn is_demo(self) -> bool {
        matches!(self, Environment::Demo)
    }
}

/// Session manager for handling authentication and session lifecycle
pub struct SessionManager {
    client: IGRestClient,
    last_refresh: DateTime<Utc>,
}

impl SessionManager {
    /// Create a new session manager by logging in with provided credentials
    pub async fn login(
        api_key: String,
        identifier: String,
        password: String,
        environment: Environment,
    ) -> Result<Self, anyhow::Error> {
        info!(
            "Creating session with environment: {}",
            if environment.is_demo() {
                "Demo"
            } else {
                "Production"
            }
        );

        let client = IGRestClient::new(api_key, identifier, password, environment.is_demo()).await?;

        let session_manager = Self {
            client,
            last_refresh: Utc::now(),
        };

        info!("Session created successfully");
        Ok(session_manager)
    }

    /// Refresh session if it's close to expiry
    ///
    /// # Arguments
    ///
    /// * `refresh_interval_mins` - Interval in minutes after which to refresh the session
    pub async fn refresh_if_needed(&mut self, refresh_interval_mins: u64) -> Result<(), anyhow::Error> {
        let now = Utc::now();
        let time_since_refresh = now.signed_duration_since(self.last_refresh);
        let refresh_threshold = Duration::from_secs(refresh_interval_mins * 60);

        if time_since_refresh.to_std()? >= refresh_threshold {
            debug!(
                "Session refresh needed (last refresh was {:?} ago)",
                time_since_refresh
            );
            self.client.refresh_session().await?;
            self.last_refresh = Utc::now();
            info!("Session refreshed successfully");
        } else {
            debug!(
                "Session refresh not needed yet (last refresh was {:?} ago)",
                time_since_refresh
            );
        }

        Ok(())
    }

    /// Get a reference to the authenticated REST client
    pub fn get_client(&self) -> &IGRestClient {
        &self.client
    }

    /// Get a mutable reference to the authenticated REST client
    pub fn get_client_mut(&mut self) -> &mut IGRestClient {
        &mut self.client
    }

    /// Logout and close the session
    pub async fn logout(self) -> Result<(), anyhow::Error> {
        info!("Logging out session");
        self.client.logout().await?;
        info!("Session logout completed");
        Ok(())
    }

    /// Get the timestamp of the last session refresh
    pub fn last_refresh_time(&self) -> DateTime<Utc> {
        self.last_refresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_is_demo() {
        assert!(Environment::Demo.is_demo());
        assert!(!Environment::Production.is_demo());
    }
}
