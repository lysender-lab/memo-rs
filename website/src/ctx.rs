use yaas::actor::Actor;

#[derive(Clone)]
pub struct Ctx {
    token: Option<String>,
    actor: Actor,
}

impl Ctx {
    pub fn new(token: Option<String>, actor: Actor) -> Self {
        Ctx { token, actor }
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some() && self.actor.has_auth_scope()
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self {
            token: None,
            actor: Actor::default(),
        }
    }
}
