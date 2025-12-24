pub mod bluesky;
pub mod github;
pub mod google;
pub mod linkedin;
pub mod plain;

pub use bluesky::BlueskyProvider;
pub use github::GitHubProvider;
pub use google::GoogleProvider;
pub use linkedin::LinkedInProvider;
pub use plain::PlainLoginProvider;
