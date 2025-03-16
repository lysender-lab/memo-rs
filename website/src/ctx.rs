use crate::models::Actor;

#[derive(Clone)]
pub struct Ctx {
    token: String,
    actor: Actor,
}

impl Ctx {
    pub fn new(token: String, actor: Actor) -> Self {
        Ctx { token, actor }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn actor(&self) -> &Actor {
        &self.actor
    }
}
