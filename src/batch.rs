use crate::client::AsyncCisClientTrait;
use cis_profile::schema::Profile;
use failure::Error;
use futures::stream::Stream;
use futures::Async;
use futures::Future;
use futures::Poll;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NextPage {
    pub id: String,
}

#[derive(Debug)]
pub struct Batch {
    pub items: Vec<Profile>,
    pub next_page: Option<NextPage>,
}

#[derive(PartialEq)]
enum ProfileIterState {
    Uninitalized,
    Inflight,
    Done,
}

/// Iterator over batches of [Profile]s.
/// Internally this retrieves batches of users from the `/users' endpoint.
pub struct AsyncProfileIter<T> {
    cis_client: T,
    filter: Option<String>,
    next: Arc<Mutex<Option<NextPage>>>,
    state: ProfileIterState,
}

impl<T> AsyncProfileIter<T> {
    pub fn new(cis_client: T, filter: Option<String>) -> Self {
        AsyncProfileIter {
            cis_client,
            filter,
            next: Arc::new(Mutex::new(None)),
            state: ProfileIterState::Uninitalized,
        }
    }
}

impl<T: AsyncCisClientTrait> Stream for AsyncProfileIter<T> {
    type Item = Vec<Profile>;
    type Error = Error;
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.state {
            ProfileIterState::Done => Ok(Async::Ready(None)),
            ProfileIterState::Uninitalized => {
                let next = Arc::clone(&self.next);
                self.state = ProfileIterState::Inflight;
                self.cis_client
                    .get_batch(&None, &self.filter)
                    .map(|batch| {
                        if batch.next_page.is_none() && batch.items.is_empty() {
                            None
                        } else {
                            println!("updated init");
                            *next.lock().unwrap() = batch.next_page;
                            Some(batch.items)
                        }
                    })
                    .poll()
            }
            ProfileIterState::Inflight => {
                println!("inflight");
                let next = Arc::clone(&self.next);
                let nexter = self.next.lock().unwrap().clone();
                if nexter.is_none() {
                    println!("done");
                    self.state = ProfileIterState::Done;
                    self.poll()
                } else {
                    self.cis_client
                        .get_batch(&nexter, &self.filter)
                        .map(|batch| {
                            println!("updated");
                            *next.lock().unwrap() = batch.next_page;
                            Some(batch.items)
                        })
                        .poll()
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
    use futures::future;
    use serde_json::Value;

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
        ) -> Box<dyn Future<Item = Profile, Error = Error>> {
            unimplemented!()
        }
        fn get_inactive_user_by(
            &self,
            _id: &str,
            _by: &GetBy,
            _filter: Option<&str>,
        ) -> Box<dyn Future<Item = Profile, Error = Error>> {
            unimplemented!()
        }
        fn get_users_iter(
            &self,
            _filter: Option<&str>,
        ) -> Box<dyn Stream<Item = Self::PI, Error = Error>> {
            unimplemented!()
        }
        fn get_batch(
            &self,
            pagination_token: &Option<NextPage>,
            _: &Option<String>,
        ) -> Box<dyn Future<Item = Batch, Error = Error>> {
            if pagination_token.is_none() && self.count == 0 {
                return Box::new(future::ok(Batch {
                    items: vec![],
                    next_page: None,
                }));
            };
            let left = if let Some(n) = pagination_token {
                n.id.parse().unwrap()
            } else {
                self.count
            };
            return Box::new(future::ok(Batch {
                items: vec![Profile::default()],
                next_page: if left > 1 {
                    Some(NextPage {
                        id: format!("{}", left - 1),
                    })
                } else {
                    None
                },
            }));
        }
        fn update_user(
            &self,
            _id: &str,
            _profile: Profile,
        ) -> Box<dyn Future<Item = Value, Error = Error>> {
            unimplemented!()
        }
        fn update_users(
            &self,
            _profiles: &[Profile],
        ) -> Box<dyn Future<Item = Value, Error = Error>> {
            unimplemented!()
        }
        fn delete_user(
            &self,
            _id: &str,
            _profile: Profile,
        ) -> Box<dyn Future<Item = Value, Error = Error>> {
            unimplemented!()
        }
        fn get_secret_store(&self) -> &SecretStore {
            unimplemented!()
        }
    }

    #[test]
    fn test_profile_iter_empty() -> Result<(), Error> {
        let mut iter = AsyncProfileIter::new(CisClientFaker { count: 0 }, None).wait();
        assert!(iter.next().is_none());
        Ok(())
    }

    #[test]
    fn test_profile_iter1() -> Result<(), Error> {
        let mut iter = AsyncProfileIter::new(CisClientFaker { count: 1 }, None).wait();
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
        Ok(())
    }

    #[test]
    fn test_profile_iter2() -> Result<(), Error> {
        let mut iter = AsyncProfileIter::new(CisClientFaker { count: 2 }, None).wait();
        assert!(iter.next().is_some());
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
        Ok(())
    }
}
