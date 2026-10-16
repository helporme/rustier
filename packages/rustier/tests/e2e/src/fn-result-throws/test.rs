pub struct Rejection {
    pub code: i32,
}

impl Rejection {
    pub fn retryable(&self) -> bool {
        self.code == 503
    }
}

pub enum AccessFailure {
    Denied { resource: String },
    Unavailable,
}

type RequestResult<T> = Result<T, String>;

pub struct Attempts {
    pub count: i32,
}

impl Attempts {
    pub fn reject(&mut self) -> Result<(), String> {
        self.count += 1;

        Err("rejected".to_owned())
    }

    pub fn record(&mut self) -> Result<i32, String> {
        self.count += 1;

        Ok(self.count)
    }
}

pub fn reject_number() -> Result<i32, i32> {
    Err(404)
}

pub fn reject_text() -> RequestResult<i32> {
    Err("offline".to_owned())
}

pub fn reject_flag() -> Result<i32, bool> {
    Err(false)
}

pub fn reject_unit() -> Result<i32, ()> {
    Err(())
}

pub fn reject_request() -> Result<(), Rejection> {
    Err(Rejection { code: 503 })
}

pub fn reject_access(unavailable: bool) -> Result<(), AccessFailure> {
    if unavailable {
        Err(AccessFailure::Unavailable)
    } else {
        Err(AccessFailure::Denied {
            resource: "invoices".to_owned(),
        })
    }
}

pub fn make_attempts() -> Attempts {
    Attempts { count: 0 }
}

pub fn reject_pair(first: &mut Attempts, second: &mut Attempts) -> Result<i32, String> {
    first.count += 1;
    second.count += 1;

    Err("pair rejected".to_owned())
}
