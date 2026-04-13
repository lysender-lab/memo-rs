use yaas::actor::Actor;

#[derive(Clone)]
pub struct Ctx {
    pub value: Option<CtxValue>,
}

#[derive(Clone)]
pub struct CtxValue {
    token: String,
    actor: Actor,
}

impl Ctx {
    pub fn new(value: Option<CtxValue>) -> Self {
        Ctx { value }
    }

    pub fn token(&self) -> Option<&str> {
        if let Some(value) = self.value.as_ref() {
            return Some(value.token.as_str());
        }
        None
    }

    pub fn actor(&self) -> Option<&Actor> {
        if let Some(value) = self.value.as_ref() {
            return Some(&value.actor);
        }
        None
    }
}

impl CtxValue {
    pub fn new(token: String, actor: Actor) -> Self {
        Self { token, actor }
    }
}
