use crate::error::ProfileIterError;
use crate::sync::client::CisClientTrait;
use cis_profile::schema::Profile;
use failure::Error;
use serde::Deserialize;
use serde::Serialize;
use std::iter::Iterator;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NextPage {
    pub id: String,
}

#[derive(Debug)]
pub struct Batch {
    pub items: Option<Vec<Profile>>,
    pub next_page: Option<NextPage>,
}

#[derive(PartialEq)]
enum ProfileIterState {
    Uninitalized,
    Inflight,
    Done,
    Error,
}

/// Iterator over batches of [Profile]s.
/// Internally this retrieves batches of users from the `/users' endpoint.
pub struct ProfileIter<T> {
    cis_client: T,
    filter: Option<String>,
    current_batch: Option<Batch>,
    state: ProfileIterState,
}

impl<T> ProfileIter<T> {
    pub fn new(cis_client: T, filter: Option<String>) -> Self {
        ProfileIter {
            cis_client,
            filter,
            current_batch: None,
            state: ProfileIterState::Uninitalized,
        }
    }
}

impl<T: CisClientTrait> Iterator for ProfileIter<T> {
    type Item = Result<Vec<Profile>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.state {
            ProfileIterState::Done | ProfileIterState::Error => None,
            ProfileIterState::Uninitalized => {
                match self.cis_client.get_batch(&None, &self.filter) {
                    Ok(new_batch) => {
                        self.state = ProfileIterState::Inflight;
                        self.current_batch = Some(new_batch);
                        self.next()
                    }
                    Err(e) => {
                        self.state = ProfileIterState::Error;
                        Some(Err(e))
                    }
                }
            }
            ProfileIterState::Inflight => {
                if let Some(batch) = &mut self.current_batch {
                    if let Some(profiles) = batch.items.take() {
                        Some(Ok(profiles))
                    } else if let Some(next_page) = batch.next_page.take() {
                        match self.cis_client.get_batch(&Some(next_page), &self.filter) {
                            Ok(new_batch) => {
                                self.current_batch = Some(new_batch);
                                self.next()
                            }
                            Err(e) => {
                                self.state = ProfileIterState::Error;
                                Some(Err(e))
                            }
                        }
                    } else {
                        self.state = ProfileIterState::Done;
                        None
                    }
                } else {
                    self.state = ProfileIterState::Error;
                    Some(Err(ProfileIterError::InvalidState.into()))
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
    use serde_json::Value;

    struct CisClientFaker {
        count: usize,
    }
    impl CisClientTrait for CisClientFaker {
        type PI = ProfileIter<Self>;
        fn get_user_by(&self, _: &str, _: &GetBy, _: Option<&str>) -> Result<Profile, Error> {
            unimplemented!()
        }
        fn get_any_user_by(&self, _: &str, _: &GetBy, _: Option<&str>) -> Result<Profile, Error> {
            unimplemented!()
        }
        fn get_inactive_user_by(
            &self,
            _: &str,
            _: &GetBy,
            _: Option<&str>,
        ) -> Result<Profile, Error> {
            unimplemented!()
        }
        fn get_users_iter(&self, _: Option<&str>) -> Result<Self::PI, Error> {
            unimplemented!()
        }
        fn get_batch(
            &self,
            pagination_token: &Option<NextPage>,
            _: &Option<String>,
        ) -> Result<Batch, Error> {
            if pagination_token.is_none() && self.count == 0 {
                return Ok(Batch {
                    items: None,
                    next_page: None,
                });
            };
            let left = if let Some(n) = pagination_token {
                n.id.parse().unwrap()
            } else {
                self.count
            };
            return Ok(Batch {
                items: Some(vec![Profile::default()]),
                next_page: if left > 1 {
                    Some(NextPage {
                        id: format!("{}", left - 1),
                    })
                } else {
                    None
                },
            });
        }
        fn update_user(&self, _: &str, _: Profile) -> Result<Value, Error> {
            unimplemented!()
        }
        fn update_users(&self, _: &[Profile]) -> Result<Value, Error> {
            unimplemented!()
        }
        fn delete_user(&self, _: &str, _: Profile) -> Result<Value, Error> {
            unimplemented!()
        }
        fn get_secret_store(&self) -> &SecretStore {
            unimplemented!()
        }
    }

    #[test]
    fn test_profile_iter_empty() -> Result<(), Error> {
        let mut iter = ProfileIter::new(CisClientFaker { count: 0 }, None);
        assert!(iter.next().is_none());
        Ok(())
    }

    #[test]
    fn test_profile_iter1() -> Result<(), Error> {
        let mut iter = ProfileIter::new(CisClientFaker { count: 1 }, None);
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
        Ok(())
    }

    #[test]
    fn test_profile_iter2() -> Result<(), Error> {
        let mut iter = ProfileIter::new(CisClientFaker { count: 2 }, None);
        assert!(iter.next().is_some());
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
        Ok(())
    }
}
