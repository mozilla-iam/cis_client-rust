use crate::client::AsyncCisClientTrait;
use cis_profile::schema::Profile;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::stream::Stream;
use futures::task::Context;
use futures::task::Poll;
use log::error;
use serde::Deserialize;
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NextPage {
    pub id: String,
}

#[derive(Debug)]
pub struct Batch {
    pub items: Vec<Profile>,
    pub next_page: Option<NextPage>,
}

#[derive(PartialEq, Clone)]
enum ProfileIterState {
    Uninitalized,
    Inflight,
    Done,
}

/// Iterator over batches of [Profile]s.
/// Internally this retrieves batches of users from the `/users' endpoint.
pub struct AsyncProfileIter<T: AsyncCisClientTrait> {
    cis_client: T,
    filter: Option<String>,
    next: Arc<Mutex<Option<NextPage>>>,
    state: Arc<RwLock<ProfileIterState>>,
}

impl<T: AsyncCisClientTrait> AsyncProfileIter<T> {
    pub fn new(cis_client: T, filter: Option<String>) -> Self {
        AsyncProfileIter {
            cis_client,
            filter,
            next: Arc::new(Mutex::new(None)),
            state: Arc::new(RwLock::new(ProfileIterState::Uninitalized)),
        }
    }
}

impl<T: AsyncCisClientTrait> Stream for AsyncProfileIter<T> {
    type Item = Vec<Profile>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let state = Arc::clone(&self.state);
        let state = (*state.read().unwrap()).clone();
        let state_update = Arc::clone(&self.state);
        match state {
            ProfileIterState::Done => Poll::Ready(None),
            ProfileIterState::Uninitalized => {
                let next = Arc::clone(&self.next);
                *state_update.write().unwrap() = ProfileIterState::Inflight;
                Future::poll(
                    Pin::new(
                        &mut self
                            .cis_client
                            .get_batch(&None, &self.filter)
                            .map_ok(|batch| {
                                if batch.next_page.is_none() && batch.items.is_empty() {
                                    None
                                } else {
                                    println!("updated init");
                                    *next.lock().unwrap() = batch.next_page;
                                    Some(batch.items)
                                }
                            })
                            .map(|res| match res {
                                Ok(items) => items,
                                Err(e) => {
                                    error!("batch error: {}", e);
                                    None
                                }
                            }),
                    ),
                    cx,
                )
            }
            ProfileIterState::Inflight => {
                println!("inflight");
                let next = Arc::clone(&self.next);
                let nexter = self.next.lock().unwrap().clone();
                if nexter.is_none() {
                    println!("done");
                    *state_update.write().unwrap() = ProfileIterState::Done;
                    self.poll_next(cx)
                } else {
                    Future::poll(
                        Pin::new(
                            &mut self
                                .cis_client
                                .get_batch(&nexter, &self.filter)
                                .map_ok(|batch| {
                                    println!("updated");
                                    *next.lock().unwrap() = batch.next_page;
                                    Some(batch.items)
                                })
                                .map(|res| match res {
                                    Ok(items) => items,
                                    Err(e) => {
                                        error!("batch error: {}", e);
                                        None
                                    }
                                }),
                        ),
                        cx,
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::getby::GetBy;
    use cis_profile::crypto::SecretStore;
    use failure::Error;
    use futures::executor::block_on;
    use futures::future;
    use futures::FutureExt;
    use futures::StreamExt;
    use serde_json::Value;
    use std::future::Future;

    struct CisClientFaker {
        count: usize,
    }
    impl AsyncCisClientTrait for CisClientFaker {
        type PI = AsyncProfileIter<Self>;
        fn get_user_by(
            &self,
            _id: &str,
            _by: &GetBy,
            _filter: Option<&str>,
        ) -> Box<dyn Future<Output = Result<Profile, Error>>> {
            unimplemented!()
        }
        fn get_inactive_user_by(
            &self,
            _id: &str,
            _by: &GetBy,
            _filter: Option<&str>,
        ) -> Box<dyn Future<Output = Result<Profile, Error>>> {
            unimplemented!()
        }
        fn get_users_iter(&self, _filter: Option<&str>) -> Box<dyn Stream<Item = Self::PI>> {
            unimplemented!()
        }
        fn get_batch(
            &self,
            pagination_token: &Option<NextPage>,
            _: &Option<String>,
        ) -> Pin<Box<dyn Future<Output = Result<Batch, Error>>>> {
            if pagination_token.is_none() && self.count == 0 {
                return future::ok(Batch {
                    items: vec![],
                    next_page: None,
                })
                .boxed();
            };
            let left = if let Some(n) = pagination_token {
                n.id.parse().unwrap()
            } else {
                self.count
            };
            return future::ok(Batch {
                items: vec![Profile::default()],
                next_page: if left > 1 {
                    Some(NextPage {
                        id: format!("{}", left - 1),
                    })
                } else {
                    None
                },
            })
            .boxed();
        }
        fn update_user(
            &self,
            _id: &str,
            _profile: Profile,
        ) -> Box<dyn Future<Output = Result<Value, Error>>> {
            unimplemented!()
        }
        fn update_users(
            &self,
            _profiles: &[Profile],
        ) -> Box<dyn Future<Output = Result<Value, Error>>> {
            unimplemented!()
        }
        fn delete_user(
            &self,
            _id: &str,
            _profile: Profile,
        ) -> Box<dyn Future<Output = Result<Value, Error>>> {
            unimplemented!()
        }
        fn get_secret_store(&self) -> &SecretStore {
            unimplemented!()
        }
    }

    #[test]
    fn test_profile_iter_empty() {
        let v: Vec<Vec<Profile>> =
            block_on(AsyncProfileIter::new(CisClientFaker { count: 0 }, None).collect());
        assert!(v.is_empty());
    }

    #[test]
    fn test_profile_iter1() {
        let v: Vec<Vec<Profile>> =
            block_on(AsyncProfileIter::new(CisClientFaker { count: 1 }, None).collect());
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn test_profile_iter10() -> Result<(), Error> {
        let v: Vec<Vec<Profile>> =
            block_on(AsyncProfileIter::new(CisClientFaker { count: 10 }, None).collect());
        assert_eq!(v.len(), 10);
        Ok(())
    }
}
