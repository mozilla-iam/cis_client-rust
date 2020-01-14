#[allow(dead_code)]
pub enum GetBy {
    Uuid,
    UserId,
    PrimaryEmail,
    PrimaryUsername,
}

impl GetBy {
    pub fn as_str(self: &GetBy) -> &'static str {
        match self {
            GetBy::Uuid => "uuid/",
            GetBy::UserId => "user_id/",
            GetBy::PrimaryEmail => "primary_email/",
            GetBy::PrimaryUsername => "primary_username/",
        }
    }
}
