pub mod client;
pub mod models;

pub use client::GathersClient;

/// RAII guard that removes registered collections from the server on drop.
///
/// Uses `tokio::runtime::Handle::current().block_on` so it works inside an
/// async context without requiring a separate runtime.
pub struct CollectionGuard<'a> {
    client: &'a GathersClient,
    collections: Vec<String>,
}

impl<'a> CollectionGuard<'a> {
    pub fn new(client: &'a GathersClient) -> Self {
        Self { client, collections: Vec::new() }
    }

    pub fn register(&mut self, name: impl Into<String>) {
        self.collections.push(name.into());
    }
}

impl Drop for CollectionGuard<'_> {
    fn drop(&mut self) {
        let handle = tokio::runtime::Handle::current();
        // block_in_place moves this thread off the async executor so block_on
        // doesn't deadlock when Drop is called from within an async context.
        tokio::task::block_in_place(|| {
            for col in &self.collections {
                handle.block_on(self.client.remove_collection(col)).ok();
            }
        });
    }
}
